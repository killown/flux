use std::path::PathBuf;

#[derive(Default, Clone)]
pub struct DirStats {
    /// Sum of file sizes in bytes.
    pub total_bytes: u64,
    /// Number of files (not directories) visited.
    pub total_files: usize,
    /// Number of directories visited (excluding `target` itself).
    pub total_dirs: usize,
    /// Wall-clock duration of the scan, in milliseconds.
    pub duration_ms: u128,
    /// Top 10 extensions by total byte size, as `(ext, count, bytes)`.
    ///
    /// Currently unreferenced by the UI but computed because it's cheap and
    /// the extension map is being built anyway.
    #[allow(dead_code)]
    pub top_exts: Vec<(String, usize, u64)>,
    /// Top 100 largest files, as `(path, bytes)`, sorted descending.
    pub largest_files: Vec<(PathBuf, u64)>,
}
