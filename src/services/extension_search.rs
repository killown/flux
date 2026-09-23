use crate::model::{AppMsg, FluxApp};
use crate::ui::paste_ops::NEXT_TASK_ID;
use aho_corasick::AhoCorasick;
use gtk::gio::prelude::*;
use ignore::{ParallelVisitor, ParallelVisitorBuilder, WalkBuilder, WalkState};
use regex::bytes::{RegexSet, RegexSetBuilder};
use relm4::prelude::*;
use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, OnceLock};

/// Results are flushed to the UI in batches of this size to avoid flooding
/// the GTK main loop with individual messages (critical for patterns like
/// `*.png` that can match tens of thousands of files).
const BATCH_SIZE: usize = 500;
const FLUSH_INTERVAL_MS: u64 = 16;

/// Static AhoCorasick matcher for forbidden system and virtual paths.
static FORBIDDEN_PATHS: OnceLock<AhoCorasick> = OnceLock::new();

fn forbidden_matcher() -> &'static AhoCorasick {
    FORBIDDEN_PATHS.get_or_init(|| {
        AhoCorasick::builder()
            .build([
                b"/proc/".as_slice(),
                b"/sys/".as_slice(),
                b"/dev/".as_slice(),
                b"/run/".as_slice(),
                b"/var/run/".as_slice(),
                b"/dosdevices/".as_slice(),
                b"/Prefixes/".as_slice(),
                b"/compatdata/".as_slice(),
                b"/drive_c/".as_slice(),
            ])
            .expect("valid forbidden path patterns")
    })
}

/// A single matched file, carried inside [`AppMsg::ExtensionSearchBatch`].
#[derive(Debug, Clone)]
pub struct ExtensionMatch {
    pub path: std::path::PathBuf,
    /// Display string shown in the result row (relative path from search root).
    pub display: String,
    pub size: u64,
    pub mtime: i64,
}

/// Optional constraints from the Advanced Search dialog.
///
/// When `None` fields are present the corresponding predicate is skipped,
/// so a default `AdvancedSearchParams` behaves identically to the plain
/// `start_extension_search` path.
#[derive(Debug, Clone, Default)]
pub struct AdvancedSearchParams {
    /// Glob patterns (already expanded from MIME shorthands) or regex rules.
    pub patterns: Vec<String>,
    /// Exclude files whose mtime is older than `now - date_seconds`.
    pub date_seconds: Option<u64>,
    /// `(larger_than, threshold_bytes)`:
    /// * `true`  → keep only files *larger than* the threshold
    /// * `false` → keep only files *smaller than* the threshold
    pub size_bytes: Option<(bool, u64)>,
    /// When `true`, dotfiles are included even if the global toggle is off.
    pub include_hidden: bool,
    /// Maximum matches allowed during the search walk before stopping.
    pub max_results: usize,
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
        max_results: app.config.ui.max_search_results,
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
    let effective_max_results = if params.max_results == 0 {
        app.config.ui.max_search_results
    } else {
        params.max_results
    };
    let params = AdvancedSearchParams {
        include_hidden: effective_hidden,
        max_results: effective_max_results,
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

    // Separate regex patterns from standard glob/extension patterns.
    let (regex_patterns, glob_patterns): (Vec<String>, Vec<String>) = params
        .patterns
        .into_iter()
        .partition(|p| p.starts_with("regex:"));

    let regex_sources: Vec<String> = regex_patterns
        .iter()
        .map(|p| p.strip_prefix("regex:").unwrap_or(p).to_string())
        .collect();

    let regex_set: Option<Arc<RegexSet>> = if !regex_sources.is_empty() {
        RegexSetBuilder::new(regex_sources)
            .case_insensitive(true)
            .build()
            .ok()
            .map(Arc::new)
    } else {
        None
    };

    // Expand MIME shorthands and compile into a GlobSet.
    let expanded: Vec<String> = glob_patterns
        .iter()
        .flat_map(|p| crate::utils::glob::expand_mime_category(p))
        .collect();

    let globset = if expanded.is_empty() {
        None
    } else {
        crate::utils::glob::compile_patterns(&expanded).map(Arc::new)
    };

    if globset.is_none() && regex_set.is_none() {
        return;
    }

    // Cancel any previous search.
    if let Some(cancellable) = app.content_search_cancellable.take() {
        cancellable.cancel();
    }

    app.is_content_searching = true;
    app.is_loading = true;
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
    let max_results = params.max_results;

    // Compute the mtime boundary once, outside the hot loop.
    let mtime_boundary: Option<std::time::SystemTime> = date_seconds
        .map(|secs| std::time::SystemTime::now() - std::time::Duration::from_secs(secs));

    relm4::spawn_blocking(move || {
        let mut builder = WalkBuilder::new(&current_dir);
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().saturating_sub(1).max(1))
            .unwrap_or(2);

        builder
            .threads(threads)
            .hidden(!include_hidden)
            .parents(true)
            .ignore(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .follow_links(true)
            .same_file_system(false);

        let matcher = forbidden_matcher();
        let walker = builder
            .filter_entry(move |entry| {
                let bytes = crate::utils::osstr_to_bytes(entry.path().as_os_str());
                !matcher.is_match(bytes)
            })
            .build_parallel();

        let (tx, rx) = mpsc::channel::<ExtensionMatch>();
        let total_count = Arc::new(AtomicUsize::new(0));

        let sender_for_collector = sender.clone();
        let load_id_for_collector = load_id.clone();
        let cancellable_for_collector = cancellable.clone();
        let collector_handle = std::thread::spawn(move || {
            let mut batch: Vec<ExtensionMatch> = Vec::with_capacity(BATCH_SIZE);
            let mut last_flush = std::time::Instant::now();
            let timeout = std::time::Duration::from_millis(FLUSH_INTERVAL_MS);

            loop {
                match rx.recv_timeout(timeout) {
                    Ok(item) => {
                        if cancellable_for_collector.is_cancelled()
                            || load_id_for_collector.load(Ordering::Acquire) != session_id
                        {
                            break;
                        }

                        batch.push(item);

                        let elapsed = last_flush.elapsed().as_millis() as u64;
                        if batch.len() >= BATCH_SIZE
                            || (elapsed >= FLUSH_INTERVAL_MS && !batch.is_empty())
                        {
                            let chunk =
                                std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                            sender_for_collector.input(AppMsg::ExtensionSearchBatch {
                                results: chunk,
                                session: session_id,
                            });
                            last_flush = std::time::Instant::now();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if cancellable_for_collector.is_cancelled()
                            || load_id_for_collector.load(Ordering::Acquire) != session_id
                        {
                            break;
                        }

                        if !batch.is_empty() {
                            let chunk =
                                std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                            sender_for_collector.input(AppMsg::ExtensionSearchBatch {
                                results: chunk,
                                session: session_id,
                            });
                            last_flush = std::time::Instant::now();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }

            // Flush any remaining results.
            if !batch.is_empty()
                && !cancellable_for_collector.is_cancelled()
                && load_id_for_collector.load(Ordering::Acquire) == session_id
            {
                sender_for_collector.input(AppMsg::ExtensionSearchBatch {
                    results: batch,
                    session: session_id,
                });
            }
        });

        struct SearchVisitor {
            tx: mpsc::Sender<ExtensionMatch>,
            current_dir: std::path::PathBuf,
            globset: Option<Arc<globset::GlobSet>>,
            regex_set: Option<Arc<RegexSet>>,
            mtime_boundary: Option<std::time::SystemTime>,
            size_bytes: Option<(bool, u64)>,
            cancellable: gtk::gio::Cancellable,
            load_id: Arc<AtomicU64>,
            session_id: u64,
            total_count: Arc<AtomicUsize>,
            max_results: usize,
            visited_inodes: HashSet<(u64, u64)>,
        }

        impl ParallelVisitor for SearchVisitor {
            fn visit(&mut self, result: Result<ignore::DirEntry, ignore::Error>) -> WalkState {
                if self.load_id.load(Ordering::Acquire) != self.session_id
                    || self.cancellable.is_cancelled()
                    || self.total_count.load(Ordering::Relaxed) >= self.max_results
                {
                    return WalkState::Quit;
                }

                let entry = match result {
                    Ok(e) => e,
                    Err(_) => return WalkState::Continue,
                };

                // Only inspect regular files.
                if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                    return WalkState::Continue;
                }

                let file_name = entry.file_name();
                let name_bytes = crate::utils::osstr_to_bytes(file_name);
                let mut matches = false;

                // Check GlobSet matches if present (fast path: stack buffer lowercasing for ASCII)
                if let Some(ref gs) = self.globset {
                    let mut lower_buf = [0u8; 256];
                    let matches_glob = if name_bytes.len() <= lower_buf.len() {
                        let buf = &mut lower_buf[..name_bytes.len()];
                        buf.copy_from_slice(name_bytes);
                        buf.make_ascii_lowercase();
                        if let Ok(s) = std::str::from_utf8(buf) {
                            gs.is_match(s)
                        } else {
                            let name_lower = file_name.to_string_lossy().to_lowercase();
                            gs.is_match(&name_lower)
                        }
                    } else {
                        let name_lower = file_name.to_string_lossy().to_lowercase();
                        gs.is_match(&name_lower)
                    };
                    if matches_glob {
                        matches = true;
                    }
                }

                // Check consolidated RegexSet match on raw bytes without string allocations
                if !matches {
                    if let Some(ref rs) = self.regex_set {
                        if rs.is_match(name_bytes) {
                            matches = true;
                        }
                    }
                }

                if !matches {
                    return WalkState::Continue;
                }

                if entry.path_is_symlink() {
                    if let Ok(meta) = entry.metadata() {
                        let key = (meta.dev(), meta.ino());
                        if !self.visited_inodes.insert(key) {
                            return WalkState::Continue;
                        }
                    }
                }

                let path = entry.into_path();

                // ── Advanced predicates ──────────────────────────────────────
                // Only call std::fs::metadata when at least one predicate is active.
                let mut precomputed_meta = None;
                if self.mtime_boundary.is_some() || self.size_bytes.is_some() {
                    match std::fs::metadata(&path) {
                        Ok(meta) => {
                            if let Some(boundary) = self.mtime_boundary {
                                match meta.modified() {
                                    Ok(mtime) if mtime < boundary => return WalkState::Continue,
                                    Err(_) => return WalkState::Continue,
                                    _ => {}
                                }
                            }
                            if let Some((larger, threshold)) = self.size_bytes {
                                let file_size = meta.len();
                                if larger && file_size <= threshold {
                                    return WalkState::Continue;
                                }
                                if !larger && file_size >= threshold {
                                    return WalkState::Continue;
                                }
                            }
                            precomputed_meta = Some(meta);
                        }
                        Err(_) => return WalkState::Continue,
                    }
                }

                let (size, mtime) = if let Some(meta) = precomputed_meta {
                    let sz = meta.len();
                    let mt = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    (sz, mt)
                } else {
                    (0, 0)
                };

                let rel = path
                    .strip_prefix(&self.current_dir)
                    .map(crate::utils::strip_current_dir)
                    .unwrap_or(&path);
                let display = rel.to_string_lossy().into_owned();

                self.total_count.fetch_add(1, Ordering::Relaxed);
                let _ = self.tx.send(ExtensionMatch {
                    path,
                    display,
                    size,
                    mtime,
                });

                WalkState::Continue
            }
        }

        struct SearchVisitorBuilder {
            tx: mpsc::Sender<ExtensionMatch>,
            current_dir: std::path::PathBuf,
            globset: Option<Arc<globset::GlobSet>>,
            regex_set: Option<Arc<RegexSet>>,
            mtime_boundary: Option<std::time::SystemTime>,
            size_bytes: Option<(bool, u64)>,
            cancellable: gtk::gio::Cancellable,
            load_id: Arc<AtomicU64>,
            session_id: u64,
            total_count: Arc<AtomicUsize>,
            max_results: usize,
        }

        impl<'s> ParallelVisitorBuilder<'s> for SearchVisitorBuilder {
            fn build(&mut self) -> Box<dyn ParallelVisitor + 's> {
                Box::new(SearchVisitor {
                    tx: self.tx.clone(),
                    current_dir: self.current_dir.clone(),
                    globset: self.globset.clone(),
                    regex_set: self.regex_set.clone(),
                    mtime_boundary: self.mtime_boundary,
                    size_bytes: self.size_bytes,
                    cancellable: self.cancellable.clone(),
                    load_id: self.load_id.clone(),
                    session_id: self.session_id,
                    total_count: self.total_count.clone(),
                    max_results: self.max_results,
                    visited_inodes: HashSet::new(),
                })
            }
        }

        let mut visitor_builder = SearchVisitorBuilder {
            tx,
            current_dir,
            globset,
            regex_set,
            mtime_boundary,
            size_bytes,
            cancellable: cancellable.clone(),
            load_id: load_id.clone(),
            session_id,
            total_count,
            max_results,
        };

        walker.visit(&mut visitor_builder);
        drop(visitor_builder);

        let _ = collector_handle.join();

        if !cancellable.is_cancelled() && load_id.load(Ordering::Acquire) == session_id {
            sender.input(AppMsg::ContentSearchDone {
                session: session_id,
            });
        }
    });
}
