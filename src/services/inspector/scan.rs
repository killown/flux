use super::stats::DirStats;
use super::visitor::StatsBuilder;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Walks `target` with `ignore`'s parallel walker and aggregates file counts,
/// byte totals, the top 10 extensions, and the 100 largest files.
///
/// Blocking, call from a `relm4::spawn_blocking` closure.
pub fn scan_directory_native(target: &Path) -> DirStats {
    let target = target
        .canonicalize()
        .unwrap_or_else(|_| target.to_path_buf());
    let start = Instant::now();

    let total_bytes = Arc::new(AtomicU64::new(0));
    let total_files = Arc::new(AtomicUsize::new(0));
    let total_dirs = Arc::new(AtomicUsize::new(0));
    let exts = Arc::new(Mutex::new(HashMap::<String, (usize, u64)>::new()));
    let largest_files = Arc::new(Mutex::new(Vec::<(PathBuf, u64)>::new()));

    let walker = ignore::WalkBuilder::new(&target)
        .hidden(false)
        .parents(false)
        .ignore(false)
        .git_ignore(false)
        .follow_links(false)
        .build_parallel();

    let mut builder = StatsBuilder::new(
        target.to_path_buf(),
        total_bytes.clone(),
        total_files.clone(),
        total_dirs.clone(),
        exts.clone(),
        largest_files.clone(),
    );

    walker.visit(&mut builder);
    drop(builder);

    let mut sorted_exts: Vec<(String, usize, u64)> = exts
        .lock()
        .unwrap()
        .drain()
        .map(|(k, (c, s))| (k, c, s))
        .collect();
    sorted_exts.sort_by_key(|a| std::cmp::Reverse(a.2));
    sorted_exts.truncate(10);

    let mut sorted_largest = largest_files.lock().unwrap().drain(..).collect::<Vec<_>>();
    sorted_largest.sort_by_key(|a| std::cmp::Reverse(a.1));
    sorted_largest.truncate(100);

    DirStats {
        total_bytes: total_bytes.load(Ordering::Relaxed),
        total_files: total_files.load(Ordering::Relaxed),
        total_dirs: total_dirs.load(Ordering::Relaxed),
        duration_ms: start.elapsed().as_millis(),
        top_exts: sorted_exts,
        largest_files: sorted_largest,
    }
}
