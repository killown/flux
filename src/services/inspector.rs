use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Default, Clone)]
pub struct DirStats {
    pub total_bytes: u64,
    pub total_files: usize,
    pub total_dirs: usize,
    pub duration_ms: u128,
    #[allow(dead_code)]
    pub top_exts: Vec<(String, usize, u64)>,
    pub largest_files: Vec<(PathBuf, u64)>,
}

pub fn scan_directory_native(target: &Path) -> DirStats {
    let start = Instant::now();
    let total_bytes = Arc::new(AtomicU64::new(0));
    let total_files = Arc::new(AtomicUsize::new(0));
    let total_dirs = Arc::new(AtomicUsize::new(0));

    let exts = Arc::new(Mutex::new(HashMap::<String, (usize, u64)>::new()));
    let largest_files = Arc::new(Mutex::new(Vec::<(PathBuf, u64)>::new()));

    let walker = ignore::WalkBuilder::new(target)
        .hidden(false)
        .parents(false)
        .ignore(false)
        .git_ignore(false)
        .follow_links(false)
        .build_parallel();

    struct StatsVisitor {
        target_dir: PathBuf,
        bytes: Arc<AtomicU64>,
        files: Arc<AtomicUsize>,
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

    struct StatsBuilder {
        target: PathBuf,
        bytes: Arc<AtomicU64>,
        files: Arc<AtomicUsize>,
        dirs: Arc<AtomicUsize>,
        exts: Arc<Mutex<HashMap<String, (usize, u64)>>>,
        largest: Arc<Mutex<Vec<(PathBuf, u64)>>>,
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

    let mut builder = StatsBuilder {
        target: target.to_path_buf(),
        bytes: total_bytes.clone(),
        files: total_files.clone(),
        dirs: total_dirs.clone(),
        exts: exts.clone(),
        largest: largest_files.clone(),
    };

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
