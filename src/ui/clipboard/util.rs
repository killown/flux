use std::path::{Path, PathBuf};

/// Generates a timestamped filename: `<stem>_YYYYMMDD_HHMMSS.<ext>`.
pub(super) fn timestamped_name(stem: &str, ext: &str) -> String {
    let now = chrono::Local::now();
    format!("{}_{}.{}", stem, now.format("%Y%m%d_%H%M%S"), ext)
}

/// Returns a path that does not exist yet by appending `_2`, `_3`, … before
/// the extension.
pub(super) fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }

    let stem = Path::new(name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ext = Path::new(name)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    for n in 2u32.. {
        let numbered = dir.join(format!("{}_{}{}", stem, n, ext));
        if !numbered.exists() {
            return numbered;
        }
    }
    candidate
}
