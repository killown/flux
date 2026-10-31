use std::path::{Path, PathBuf};

/// Derives an auto-renamed destination path that does not yet exist.
///
/// Given `/dest/file.txt` returns `/dest/file (1).txt`, `/dest/file (2).txt`, …
/// until a free slot is found.  Works for both files and directories.
pub fn auto_rename_dest(dest: &Path) -> PathBuf {
    crate::hit!("auto_rename_dest");
    let stem = dest
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ext = dest
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = dest.parent().unwrap_or(std::path::Path::new("/"));

    let mut n = 1u32;
    loop {
        let candidate = parent.join(format!("{} ({}){}", stem, n, ext));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}
