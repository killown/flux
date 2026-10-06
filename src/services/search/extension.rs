use super::forbidden::forbidden_matcher;
use super::types::{AdvancedSearchParams, ExtensionMatch};
use crate::model::{AppMsg, FluxApp};
use crate::ui::ops::paste::NEXT_TASK_ID;
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

/// Set `FLUX_DEBUG_SEARCH=1` to make the `search_debug!` macro print to
/// stderr. Read once and cached, so we pay the env lookup at most once per
/// process.
fn search_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("FLUX_DEBUG_SEARCH").is_some())
}

macro_rules! search_debug {
    ($($arg:tt)*) => {
        if search_debug_enabled() {
            eprintln!($($arg)*);
        }
    };
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
        only_folders: false,
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
    let active_tab = &mut app.tabs[app.active_tab_index];
    active_tab.files.clear();
    app.filter.clear();

    // Force list mode (same UX as content search).
    if !app.search_saved_layout {
        app.saved_list_mode = app.is_list_mode;
        app.saved_max_columns = app.tabs[app.active_tab_index].files.view.max_columns();
        app.search_saved_layout = true;
    }
    app.is_list_mode = true;
    let active_view = &app.tabs[app.active_tab_index].files.view;
    active_view.set_min_columns(1);
    active_view.set_max_columns(1);
    app.sync_list_mode();

    let cancellable = gtk::gio::Cancellable::new();
    app.content_search_cancellable = Some(cancellable.clone());

    let session_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
    app.load_id.store(session_id, Ordering::SeqCst);

    let current_dir = app.current_path.clone();
    let load_id = app.load_id.clone();

    // Snapshot the params that the walk thread needs.
    let include_hidden = params.include_hidden;
    let only_folders = params.only_folders;
    let date_seconds = params.date_seconds;
    let size_bytes = params.size_bytes;
    let max_results = params.max_results;

    let is_simple_name_search =
        regex_set.is_none() && !only_folders && date_seconds.is_none() && size_bytes.is_none();

    // Compute the mtime boundary once, outside the hot loop.
    let mtime_boundary: Option<std::time::SystemTime> = date_seconds
        .map(|secs| std::time::SystemTime::now() - std::time::Duration::from_secs(secs));

    relm4::spawn_blocking(move || {
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

        let mut index_hit = false;
        let indexer_ready = crate::services::search::indexer::is_ready();

        if is_simple_name_search && indexer_ready {
            if let Some(first_pattern) = glob_patterns.first() {
                let clean: String = first_pattern
                    .replace(['*', '?'], " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                search_debug!(
                    "[Search Debug] first_pattern={:?} clean={:?}",
                    first_pattern,
                    clean
                );
                if clean.len() >= 3 {
                    match crate::services::search::indexer::SearchIndex::open_or_create() {
                        Ok(index) => {
                            let query_limit = if max_results == 0 { 500 } else { max_results };

                            let words: Vec<&str> = clean.split_whitespace().collect();
                            let mut candidates: Vec<std::path::PathBuf> = Vec::new();
                            {
                                let mut seen = std::collections::HashSet::new();
                                for word in &words {
                                    if word.len() < 3 {
                                        continue;
                                    }
                                    if let Ok(hits) =
                                        index.query_in(word, Some(&current_dir), query_limit)
                                    {
                                        for h in hits {
                                            if seen.insert(h.clone()) {
                                                candidates.push(h);
                                            }
                                        }
                                    }
                                }
                            }
                            search_debug!(
                                "[Search Debug] FTS per-word union: {} candidates for '{}'",
                                candidates.len(),
                                clean
                            );

                            let paths: Vec<std::path::PathBuf> = if !candidates.is_empty() {
                                use nucleo::pattern::{CaseMatching, Normalization, Pattern};
                                use nucleo::Utf32Str;
                                let parsed = Pattern::parse(
                                    &clean,
                                    CaseMatching::Ignore,
                                    Normalization::Smart,
                                );
                                let mut matcher = nucleo::Matcher::new(nucleo::Config::DEFAULT);
                                let mut buf: Vec<char> = Vec::with_capacity(256);
                                let mut scored: Vec<(u32, std::path::PathBuf)> = Vec::new();
                                for p in candidates {
                                    let name =
                                        p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                                    buf.clear();
                                    let utf32 = Utf32Str::new(name, &mut buf);
                                    if let Some(s) = parsed.score(utf32, &mut matcher) {
                                        scored.push((s, p));
                                    }
                                }
                                scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
                                scored.truncate(query_limit);
                                scored.into_iter().map(|(_, p)| p).collect()
                            } else {
                                search_debug!(
                                    "[Search Debug] FTS miss for '{}', falling back to fuzzy scan",
                                    clean
                                );
                                index
                                    .query_fuzzy(&clean, Some(&current_dir), query_limit)
                                    .unwrap_or_default()
                            };

                            if !paths.is_empty() {
                                search_debug!(
                                    "[Search Debug] Index returned {} paths for '{}'",
                                    paths.len(),
                                    clean
                                );
                                let mut seen_inodes: std::collections::HashSet<(u64, u64)> =
                                    std::collections::HashSet::new();
                                let mut stale = Vec::new();
                                for p in paths {
                                    if p.to_string_lossy().contains("/run/user") {
                                        stale.push(p.to_string_lossy().to_string());
                                        continue;
                                    }
                                    if let Ok(meta) = std::fs::metadata(&p) {
                                        let key = (meta.dev(), meta.ino());
                                        if !seen_inodes.insert(key) {
                                            continue;
                                        }

                                        let rel = p
                                            .strip_prefix(&current_dir)
                                            .map(crate::utils::strip_current_dir)
                                            .unwrap_or(&p);
                                        let display = rel.to_string_lossy().into_owned();
                                        let mtime = meta
                                            .modified()
                                            .ok()
                                            .and_then(|t| {
                                                t.duration_since(std::time::UNIX_EPOCH).ok()
                                            })
                                            .map(|d| d.as_secs() as i64)
                                            .unwrap_or(0);

                                        let _ = tx.send(ExtensionMatch {
                                            path: p,
                                            display,
                                            size: meta.len(),
                                            mtime,
                                        });
                                        total_count.fetch_add(1, Ordering::Relaxed);
                                    } else {
                                        stale.push(p.to_string_lossy().to_string());
                                    }
                                }

                                if !stale.is_empty() {
                                    crate::services::search::indexer::prune_paths(stale);
                                }
                                index_hit = true;
                                search_debug!(
                                    "[Search Debug] Successfully dispatched {} matches via SQLite index!",
                                    total_count.load(Ordering::Relaxed)
                                );
                            } else {
                                search_debug!(
                                    "[Search Debug] Index miss for '{}', falling back to walk",
                                    clean
                                );
                            }
                        }
                        Err(e) => {
                            search_debug!(
                                "[Search Debug] SearchIndex::open_or_create error: {:?}",
                                e
                            );
                        }
                    }
                }
            }
        }

        if !index_hit
            && !cancellable.is_cancelled()
            && load_id.load(Ordering::Acquire) == session_id
        {
            search_debug!("[Search Debug] Falling back to WalkBuilder filesystem crawl...");

            let indexed_dir_mtimes: std::collections::HashMap<std::path::PathBuf, i64> =
                if indexer_ready && is_simple_name_search {
                    let mut map = std::collections::HashMap::new();
                    let db_path = crate::services::search::indexer::SearchIndex::db_path();
                    if let Ok(conn) = rusqlite::Connection::open_with_flags(
                        &db_path,
                        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                    ) {
                        let root_str = current_dir.to_string_lossy();
                        let root_trim = root_str.trim_end_matches('/');
                        let lo = format!("{}/", root_trim);
                        let hi = format!("{}0", root_trim);
                        if let Ok(mut stmt) = conn.prepare(
                            "SELECT path, mtime FROM entries \
                             WHERE is_dir = 1 \
                               AND (path = ?1 OR (path >= ?2 AND path < ?3))",
                        ) {
                            if let Ok(rows) = stmt
                                .query_map(rusqlite::params![root_str.as_ref(), lo, hi], |row| {
                                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                                })
                            {
                                for r in rows.flatten() {
                                    map.insert(std::path::PathBuf::from(r.0), r.1);
                                }
                            }
                        }
                    }
                    map
                } else {
                    std::collections::HashMap::new()
                };

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

            let skip_stable_dirs =
                !only_folders && is_simple_name_search && !indexed_dir_mtimes.is_empty();
            let matcher = forbidden_matcher();
            let walker = builder
                .filter_entry(move |entry| {
                    let bytes = crate::utils::osstr_to_bytes(entry.path().as_os_str());
                    if matcher.is_match(bytes) {
                        return false;
                    }
                    if skip_stable_dirs
                        && entry.depth() > 0
                        && entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                    {
                        if let Some(&stored_ns) = indexed_dir_mtimes.get(entry.path()) {
                            if stored_ns != 0 {
                                if let Ok(meta) = entry.metadata() {
                                    if let Ok(mtime) = meta.modified() {
                                        if let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH)
                                        {
                                            if dur.as_nanos() as i64 == stored_ns {
                                                return false;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    true
                })
                .build_parallel();

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
                only_folders: bool,
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

                    // Check directory vs file requirements
                    let is_symlink = entry.path_is_symlink();
                    let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                        || (is_symlink && entry.path().is_dir());

                    if self.only_folders {
                        if !is_dir || entry.depth() == 0 {
                            return WalkState::Continue;
                        }
                    } else if is_dir
                        || (!entry.file_type().is_some_and(|ft| ft.is_file())
                            && !entry.path().is_file())
                    {
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
                                        Ok(mtime) if mtime < boundary => {
                                            return WalkState::Continue
                                        }
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
                only_folders: bool,
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
                        only_folders: self.only_folders,
                    })
                }
            }

            let mut visitor_builder = SearchVisitorBuilder {
                tx: tx.clone(),
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
                only_folders,
            };

            walker.visit(&mut visitor_builder);
        }

        drop(tx);
        let _ = collector_handle.join();

        if !cancellable.is_cancelled() && load_id.load(Ordering::Acquire) == session_id {
            sender.input(AppMsg::ContentSearchDone {
                session: session_id,
            });
        }
    });
}
