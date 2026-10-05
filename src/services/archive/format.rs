//! Archive format detection by extension and MIME type.

use crate::services::archive::util::lc_name;
use std::path::Path;

// ─── Format detection ─────────────────────────────────────────────────────────

/// Returns `true` for archives that `list_archive_entries` can browse.
#[allow(dead_code)]
pub fn is_browsable_archive(path: &Path) -> bool {
    if matches_extension(
        path,
        &[
            "zip", "tar", "tar.gz", "tgz", "tar.bz2", "tbz2", "tar.xz", "txz", "tar.lzma", "tlz",
            "tar.zst", "tzst", "tar.lz4", "7z", "rar", "gz", "bz2", "xz", "lzma", "zst", "zstd",
            "lz4", "iso", "deb",
        ],
    ) {
        return true;
    }

    // Fall back to MIME type for custom extension mappings (e.g. .cbz -> zip, .cbr -> rar)
    let mime = crate::utils::media::get_mime_type(path);
    matches!(
        mime.as_str(),
        "application/zip"
            | "application/x-zip-compressed"
            | "application/x-7z-compressed"
            | "application/x-tar"
            | "application/x-rar"
            | "application/x-rar-compressed"
            | "application/gzip"
            | "application/x-bzip2"
            | "application/x-xz"
            | "application/zstd"
    )
}

fn matches_extension(path: &Path, exts: &[&str]) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    exts.iter().any(|e| name.ends_with(&format!(".{e}")))
}

#[allow(dead_code)]
pub fn is_supported_archive(path: &std::path::Path) -> bool {
    let name_lc = lc_name(path);
    name_lc.ends_with(".zip")
        || name_lc.ends_with(".7z")
        || name_lc.ends_with(".rar")
        || name_lc.ends_with(".iso")
        || name_lc.ends_with(".tar")
        || name_lc.ends_with(".tar.gz")
        || name_lc.ends_with(".tgz")
        || name_lc.ends_with(".tar.bz2")
        || name_lc.ends_with(".tbz2")
        || name_lc.ends_with(".tar.xz")
        || name_lc.ends_with(".txz")
        || name_lc.ends_with(".tar.zst")
        || name_lc.ends_with(".tzst")
        || name_lc.ends_with(".tar.lz4")
        || name_lc.ends_with(".gz")
        || name_lc.ends_with(".bz2")
        || name_lc.ends_with(".xz")
        || name_lc.ends_with(".zst")
        || name_lc.ends_with(".zstd")
        || name_lc.ends_with(".lz4")
}
