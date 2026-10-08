use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Accumulates stats from one `ignore` worker thread.
///
/// The shared `Arc`s are used for the counters (lock-free atomics), and the
/// two `Mutex`-wrapped collections are only touched on drop.
pub(super) struct StatsVisitor {
    target_dir: PathBuf,
    #[allow(dead_code)]
    bytes: Arc<AtomicU64>,
    #[allow(dead_code)]
    files: Arc<AtomicUsize>,
    #[allow(dead_code)]
    dirs: Arc<AtomicUsize>,
    exts: Arc<Mutex<HashMap<String, (usize, u64)>>>,
    largest: Arc<Mutex<Vec<(PathBuf, u64)>>>,
    local_exts: HashMap<String, (usize, u64)>,
    local_largest: Vec<(PathBuf, u64)>,
}

impl ignore::ParallelVisitor for StatsVisitor {
    fn visit(&mut self, entry: Result<ignore::DirEntry, ignore::Error>) -> ignore::WalkState {
        let Ok(entry) = entry else {
            return ignore::WalkState::Continue;
        };
        let path = entry.path();
        if path == self.target_dir {
            return ignore::WalkState::Continue;
        }

        if let Ok(meta) = entry.metadata() {
            if meta.is_dir() {
                self.dirs.fetch_add(1, Ordering::Relaxed);
            } else if meta.is_file() {
                let size = meta.len();
                self.files.fetch_add(1, Ordering::Relaxed);
                self.bytes.fetch_add(size, Ordering::Relaxed);

                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_else(|| "(none)".to_string());

                let entry = self.local_exts.entry(ext).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += size;

                self.local_largest.push((path.to_path_buf(), size));
            }
        }

        ignore::WalkState::Continue
    }
}

impl Drop for StatsVisitor {
    fn drop(&mut self) {
        let mut global_exts = self.exts.lock().unwrap();
        for (ext, (count, size)) in self.local_exts.drain() {
            let e = global_exts.entry(ext).or_insert((0, 0));
            e.0 += count;
            e.1 += size;
        }

        let mut global_largest = self.largest.lock().unwrap();
        global_largest.append(&mut self.local_largest);
    }
}

/// Builds one `StatsVisitor` per `ignore` worker thread.
pub(super) struct StatsBuilder {
    pub(super) target: PathBuf,
    pub(super) bytes: Arc<AtomicU64>,
    pub(super) files: Arc<AtomicUsize>,
    pub(super) dirs: Arc<AtomicUsize>,
    pub(super) exts: Arc<Mutex<HashMap<String, (usize, u64)>>>,
    pub(super) largest: Arc<Mutex<Vec<(PathBuf, u64)>>>,
}

impl StatsBuilder {
    pub(super) fn new(
        target: PathBuf,
        bytes: Arc<AtomicU64>,
        files: Arc<AtomicUsize>,
        dirs: Arc<AtomicUsize>,
        exts: Arc<Mutex<HashMap<String, (usize, u64)>>>,
        largest: Arc<Mutex<Vec<(PathBuf, u64)>>>,
    ) -> Self {
        Self {
            target,
            bytes,
            files,
            dirs,
            exts,
            largest,
        }
    }
}

impl<'s> ignore::ParallelVisitorBuilder<'s> for StatsBuilder {
    fn build(&mut self) -> Box<dyn ignore::ParallelVisitor + 's> {
        Box::new(StatsVisitor {
            target_dir: self.target.clone(),
            bytes: self.bytes.clone(),
            files: self.files.clone(),
            dirs: self.dirs.clone(),
            exts: self.exts.clone(),
            largest: self.largest.clone(),
            local_exts: HashMap::new(),
            local_largest: Vec::new(),
        })
    }
}
