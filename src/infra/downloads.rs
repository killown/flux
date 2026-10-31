//! Detection of in-progress downloads from their scratch files.

use std::path::Path;

/// Suffixes used by download tools while a file is still being written.
///
/// Firefox: `<name>.<random>.RW2.part`
/// Chromium: `<name>.crdownload`
/// wget: `<name>.tmp`
///
/// Watcher events for paths with these suffixes are ignored entirely,
/// they are internal scratch files, never visible to the user, and
/// thumbnailing them wastes cycles on partial data.
const DOWNLOAD_TEMP_SUFFIXES: &[&str] = &["part", "crdownload", "download", "partial", "tmp"];

/// Returns `true` when `path` is a download scratch file that the user
/// should never see.
pub fn is_download_temp(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        let lower = e.to_ascii_lowercase();
        DOWNLOAD_TEMP_SUFFIXES.contains(&lower.as_str())
    })
}

/// Returns `true` when a sibling `.part` (or `.crdownload` etc.) file exists
/// in the same directory that appears to belong to `path`.
///
/// Download tools create the final path as an empty placeholder, then write
/// to `<name>.<random>.<ext>.part` next to it, and rename on completion. As
/// long as that sibling exists, the download is still in progress and the
/// final file is not safe to thumbnail.
///
/// Matching is done on the file stem: `DSC_4207.NEF` matches
/// `DSC_4207.L8Koxwkl.NEF.part` because both start with `DSC_4207`.
///
/// This reads the entire parent directory with blocking I/O, so it must not
/// run on the GTK thread.
pub fn has_active_download_sibling(path: &Path) -> bool {
    crate::hit!("has_active_download_sibling");
    let Some(parent) = path.parent() else {
        return false;
    };
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    if stem.is_empty() {
        return false;
    }

    let Ok(entries) = std::fs::read_dir(parent) else {
        return false;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Only consider siblings that look like download scratch files.
        if !is_download_temp(Path::new(name_str.as_ref())) {
            continue;
        }

        // The scratch file must share the target's stem prefix. This avoids
        // false positives from unrelated downloads in the same directory.
        if name_str.starts_with(stem) {
            return true;
        }
    }

    false
}
