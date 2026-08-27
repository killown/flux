use crate::model::{AppMsg, FluxApp};
use crate::ui::paste_ops::NEXT_TASK_ID;
use gtk::gio::prelude::*;
use ignore::WalkBuilder;
use relm4::prelude::*;
use std::sync::atomic::Ordering;

/// Results are flushed to the UI in batches of this size to avoid flooding
/// the GTK main loop with individual messages (critical for patterns like
/// `*.png` that can match tens of thousands of files).
const BATCH_SIZE: usize = 50;

/// Maximum number of search results to prevent hanging or excessive memory usage.
const MAX_SEARCH_RESULTS: usize = 5000;

/// A single matched file, carried inside [`AppMsg::ExtensionSearchBatch`].
#[derive(Debug, Clone)]
pub struct ExtensionMatch {
    pub path: std::path::PathBuf,
    /// Display string shown in the result row (relative path from search root).
    pub display: String,
}

/// Optional constraints from the Advanced Search dialog.
///
/// When `None` fields are present the corresponding predicate is skipped,
/// so a default `AdvancedSearchParams` behaves identically to the plain
/// `start_extension_search` path.
#[derive(Debug, Clone, Default)]
pub struct AdvancedSearchParams {
    /// Glob patterns (already expanded from MIME shorthands).
    pub patterns: Vec<String>,
    /// Exclude files whose mtime is older than `now - date_seconds`.
    pub date_seconds: Option<u64>,
    /// `(larger_than, threshold_bytes)`:
    /// * `true`  → keep only files *larger than* the threshold
    /// * `false` → keep only files *smaller than* the threshold
    pub size_bytes: Option<(bool, u64)>,
    /// When `true`, dotfiles are included even if the global toggle is off.
    pub include_hidden: bool,
}

/// Launch a recursive filename search from `app.current_path` using `ignore`'s
/// synchronous traversal.
///
/// Matched files are delivered in batches via [`AppMsg::ExtensionSearchBatch`]
/// so the GTK main loop is never saturated, even for patterns like `*.png`
/// that can match thousands of files. Each batch is appended to the grid in a
/// single update cycle.
///
/// Cancellation, session IDs, and list-mode layout reuse the existing
/// content-search infrastructure so navigation away cleans up automatically.
pub fn start_extension_search(
    app: &mut FluxApp,
    patterns: Vec<String>,
    sender: AsyncComponentSender<FluxApp>,
) {
    let params = AdvancedSearchParams {
        patterns,
        include_hidden: app.show_hidden,
        ..Default::default()
    };
    start_walk(app, params, sender);
}

/// Launch a recursive search with the full set of constraints from the
/// Advanced Search dialog.
pub fn start_advanced_search(
    app: &mut FluxApp,
    params: AdvancedSearchParams,
    sender: AsyncComponentSender<FluxApp>,
) {
    // When the dialog's "include hidden" toggle is off, respect the session
    // flag, when it's on, override it regardless of the global setting.
    let effective_hidden = params.include_hidden || app.show_hidden;
    let params = AdvancedSearchParams {
        include_hidden: effective_hidden,
        ..params
    };
    start_walk(app, params, sender);
}

// ── Shared implementation ────────────────────────────────────────────────────

/// Core walk implementation shared by both public entry points.
fn start_walk(
    app: &mut FluxApp,
    params: AdvancedSearchParams,
    sender: AsyncComponentSender<FluxApp>,
) {
    if params.patterns.is_empty() {
        return;
    }

    // Expand MIME shorthands and compile into a GlobSet.
    let expanded: Vec<String> = params
        .patterns
        .iter()
        .flat_map(|p| crate::utils::glob::expand_mime_category(p))
        .collect();

    let globset = match crate::utils::glob::compile_patterns(&expanded) {
        Some(gs) => gs,
        None => return,
    };

    // Cancel any previous search.
    if let Some(cancellable) = app.content_search_cancellable.take() {
        cancellable.cancel();
    }

    app.is_content_searching = true;
    app.files.clear();
    app.filter.clear();

    // Force list mode (same UX as content search).
    if !app.search_saved_layout {
        app.saved_list_mode = app.is_list_mode;
        app.saved_max_columns = app.files.view.max_columns();
        app.search_saved_layout = true;
    }
    app.is_list_mode = true;
    app.files.view.set_min_columns(1);
    app.files.view.set_max_columns(1);
    app.sync_list_mode();

    let cancellable = gtk::gio::Cancellable::new();
    app.content_search_cancellable = Some(cancellable.clone());

    let session_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
    app.load_id.store(session_id, Ordering::SeqCst);

    let current_dir = app.current_path.clone();
    let load_id = app.load_id.clone();

    // Snapshot the params that the walk thread needs.
    let include_hidden = params.include_hidden;
    let date_seconds = params.date_seconds;
    let size_bytes = params.size_bytes;

    // Compute the mtime boundary once, outside the hot loop.
    let mtime_boundary: Option<std::time::SystemTime> = date_seconds
        .map(|secs| std::time::SystemTime::now() - std::time::Duration::from_secs(secs));

    relm4::spawn_blocking(move || {
        let mut batch: Vec<ExtensionMatch> = Vec::with_capacity(BATCH_SIZE);
        let mut count: usize = 0;

        let mut builder = WalkBuilder::new(&current_dir);
        builder
            .hidden(!include_hidden)
            .parents(true)
            .ignore(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .follow_links(true)
            .same_file_system(false);

        // Filter out pseudo-filesystems, virtual Wine/Proton drives, and prefix loops.
        let walker = builder
            .filter_entry(|entry| {
                let path = entry.path();
                let s = path.to_string_lossy();
                !s.contains("/proc/")
                    && !s.contains("/sys/")
                    && !s.contains("/dev/")
                    && !s.contains("/dosdevices/")
                    && !s.contains("/Prefixes/")
                    && !s.contains("/compatdata/")
                    && !s.contains("/drive_c/")
            })
            .build();

        for result in walker {
            if load_id.load(Ordering::Acquire) != session_id
                || cancellable.is_cancelled()
                || count >= MAX_SEARCH_RESULTS
            {
                break;
            }

            let entry = match result {
                Ok(e) => e,
                Err(_) => continue,
            };

            // Only inspect regular files.
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }

            let name = entry.file_name().to_string_lossy();
            if !globset.is_match(name.to_lowercase()) {
                continue;
            }

            let path = entry.path();

            // ── Advanced predicates ──────────────────────────────────────
            // Only call std::fs::metadata when at least one predicate is active.
            if mtime_boundary.is_some() || size_bytes.is_some() {
                match std::fs::metadata(path) {
                    Ok(meta) => {
                        if let Some(boundary) = mtime_boundary {
                            match meta.modified() {
                                Ok(mtime) if mtime < boundary => continue,
                                Err(_) => continue,
                                _ => {}
                            }
                        }
                        if let Some((larger, threshold)) = size_bytes {
                            let file_size = meta.len();
                            if larger && file_size <= threshold {
                                continue;
                            }
                            if !larger && file_size >= threshold {
                                continue;
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }

            let path = entry.into_path();
            let display = path
                .strip_prefix(&current_dir)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| path.to_string_lossy().into_owned());

            batch.push(ExtensionMatch { path, display });
            count += 1;

            if batch.len() >= BATCH_SIZE {
                let chunk = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                sender.input(AppMsg::ExtensionSearchBatch {
                    results: chunk,
                    session: session_id,
                });
            }
        }

        // Flush any remaining results.
        if !batch.is_empty()
            && !cancellable.is_cancelled()
            && load_id.load(Ordering::Acquire) == session_id
        {
            sender.input(AppMsg::ExtensionSearchBatch {
                results: batch,
                session: session_id,
            });
        }

        if !cancellable.is_cancelled() && load_id.load(Ordering::Acquire) == session_id {
            sender.input(AppMsg::ContentSearchDone {
                session: session_id,
            });
        }
    });
}
