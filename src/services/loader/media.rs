/// Returns `(is_image, is_video)` by extension only - zero I/O, safe to call inside
/// a Rayon iterator on the directory listing hot path. Accuracy is intentionally
/// approximate: false positives are harmless (they go into `media_tasks` and get
/// filtered again by the full `is_visual_media` check inside `get_or_create_thumbnail`).
#[inline]
pub(super) fn is_visual_media_by_ext(path: &std::path::Path) -> (bool, bool) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    match ext.as_deref() {
        Some(
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "heic" | "heif" | "bmp" | "tiff"
            | "tif" | "jxl" | "svg" | "pdf" | "ttf" | "otf" | "woff" | "woff2" | "ttc" | "ico",
        ) => (true, false),
        Some(
            "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "wmv" | "m4v" | "mpg" | "mpeg" | "ts"
            | "ogv",
        ) => (false, true),
        _ => (false, false),
    }
}
