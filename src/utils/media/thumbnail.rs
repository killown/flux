use adw::gdk;
use gio::prelude::FileExt;
use std::fs;
use std::path::{Path, PathBuf};

use super::pdf::pdf_thumbnail;
use super::png::{mtime_secs, read_thumb_mtime};
use super::tiers::{find_cached_thumbnail, tier_for_size};

/// Retrieves a thumbnail [`gdk::Texture`] for a visual media file, generating
/// and caching it on first access.
///
/// Cache entries follow the [FreeDesktop Thumbnail Managing Standard], stored under
/// `$XDG_CACHE_HOME/thumbnails/<size-tier>/` as `MD5("file://<path>").hex + ".png"`.
/// This makes cache hits session-persistent: a thumbnail written in a previous
/// session is reused without regeneration as long as the source file's `mtime` and
/// size have not changed.
///
/// Stale entries (source modified after thumbnail was written, or zero-byte files)
/// are evicted and regenerated atomically.
///
/// [FreeDesktop Thumbnail Managing Standard]: https://specifications.freedesktop.org/thumbnail-spec/
///
/// # Arguments
///
/// * `path` - Absolute path to an image or video file.
///
/// # Returns
///
/// `Some(texture)` on success, `None` if the file is not visual media, if any
/// required external tool (`ffmpeg`, `raw-thumbnailer`) is unavailable, or if
/// I/O fails.
pub async fn get_or_create_thumbnail(path: &std::path::Path) -> Option<gtk::gdk::Texture> {
    crate::hit!("get_or_create_thumbnail");
    let path = path.to_path_buf();

    // WARNING: Never generate thumbnails for files inside the thumbnail cache itself.
    // Prevents an infinite recursive generation loop when browsing ~/.cache/thumbnails/
    if let Some(cache_root) = dirs::cache_dir() {
        if path.starts_with(cache_root.join("thumbnails")) {
            return None;
        }
    }

    tokio::task::spawn_blocking(move || generate_thumbnail_sync(&path))
        .await
        .ok()
        .flatten()
}

/// Resolves the FreeDesktop-compliant thumbnail cache path for a given source file.
///
/// [Thumbnail Managing Standard]: https://specifications.freedesktop.org/thumbnail-spec/
///
/// # Arguments
///
/// * `path` - Absolute path to the source media file.
///
/// # Returns
///
/// `(cache_dir, cache_path)` where `cache_dir` is the resolved size-tier directory
/// and `cache_path` is the full `.png` destination. Returns `None` if
/// `dirs::cache_dir()` is unavailable or `path` contains non-UTF-8 bytes.
fn thumbnail_cache_path(path: &Path, target_size: i32) -> Option<(PathBuf, PathBuf, String)> {
    let cache_dir = dirs::cache_dir()?
        .join("thumbnails")
        .join(tier_for_size(target_size));

    let uri = gio::File::for_path(path).uri();
    let hash = format!("{:x}", md5::compute(uri.as_bytes()));
    let cache_path = cache_dir.join(format!("{hash}.png"));

    Some((cache_dir, cache_path, hash))
}

/// Returns whether cached PNG bytes still match the source, using `Thumb::MTime` per the spec.
///
/// Entries without `Thumb::MTime` (older flux caches) fall back to comparing file mtimes.
fn thumbnail_is_valid(cache_meta: &fs::Metadata, png: &[u8], source_meta: &fs::Metadata) -> bool {
    if cache_meta.len() == 0 {
        return false;
    }

    if let (Some(stored), Some(current)) = (read_thumb_mtime(png), mtime_secs(source_meta)) {
        return stored == current;
    }

    match (cache_meta.modified().ok(), source_meta.modified().ok()) {
        (Some(ct), Some(st)) => st <= ct,
        _ => true,
    }
}

pub(crate) fn generate_thumbnail_sync(path: &std::path::Path) -> Option<gtk::gdk::Texture> {
    let path_str = path.to_string_lossy();
    if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
        return crate::services::archive::get_archive_thumbnail(path);
    }

    let (show_thumbnails, target_size, thumb_types, _ffmpeg_threads, _auto_rotate, _seek_secs) =
        crate::utils::config::get_thumb_config();

    if !show_thumbnails {
        return None;
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    let is_pdf_file = ext == "pdf";
    let is_font_file =
        !is_pdf_file && matches!(ext.as_str(), "ttf" | "otf" | "woff" | "woff2" | "ttc");
    let is_exe_file = !is_pdf_file && !is_font_file && ext == "exe";
    let is_audio = !is_pdf_file
        && !is_font_file
        && !is_exe_file
        && matches!(
            ext.as_str(),
            "mp3" | "flac" | "m4a" | "ogg" | "wav" | "aac" | "wma" | "opus"
        );

    // RAW files go through `raw-thumbnailer`, which extracts the camera's
    // embedded JPEG preview instead of decoding sensor data. Classified as
    // image so they honor `thumbnail_types.images` without a separate toggle.
    let is_raw_file = !is_pdf_file
        && !is_font_file
        && !is_exe_file
        && !is_audio
        && crate::utils::media::is_raw_image_ext(&ext);

    let (is_img, is_vid) =
        if !is_pdf_file && !is_font_file && !is_exe_file && !is_audio && !is_raw_file {
            match ext.as_str() {
                "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "heic" | "heif" | "bmp"
                | "tiff" | "tif" | "jxl" | "svg" | "ico" => (true, false),
                "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "wmv" | "m4v" | "mpg" | "mpeg"
                | "ts" | "ogv" => (false, true),
                _ => {
                    let filename = path.file_name().unwrap_or_default().to_string_lossy();
                    let (ct, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
                    (ct.starts_with("image/"), ct.starts_with("video/"))
                }
            }
        } else {
            (false, false)
        };

    let is_supported = (is_pdf_file && thumb_types.pdfs)
        || (is_font_file && thumb_types.fonts)
        || (is_img && thumb_types.images)
        || (is_raw_file && thumb_types.images)
        || (is_vid && thumb_types.videos)
        || (is_exe_file && thumb_types.executables)
        || (is_audio && thumb_types.audio);

    if !is_supported {
        return None;
    }

    let (cache_dir, cache_path, hash) = thumbnail_cache_path(path, target_size)?;

    let source_meta = std::fs::metadata(path).ok();

    let existing_thumbnail = cache_dir
        .parent()
        .and_then(|root| find_cached_thumbnail(root, &hash, target_size));

    if let Some((existing, meta)) = existing_thumbnail {
        if meta.len() > 1024 * 1024 {
            let _ = std::fs::remove_file(&existing);
        } else {
            if let Ok(bytes) = std::fs::read(&existing) {
                let is_valid = source_meta
                    .as_ref()
                    .map(|m| thumbnail_is_valid(&meta, &bytes, m))
                    .unwrap_or(true);

                if is_valid {
                    if let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from(&bytes)) {
                        return Some(texture);
                    }
                }
            }
            let _ = std::fs::remove_file(&existing);
        }
    }

    let _ = std::fs::create_dir_all(&cache_dir);

    if is_raw_file {
        // `raw-thumbnailer` writes PNG directly, no temp-file dance needed
        // on the caller side. If the binary is missing, fall through to None
        // and the grid shows the generic image icon.
        if !crate::utils::media::raw_thumbnailer_available() {
            return None;
        }
        return crate::utils::media::raw_thumbnail(path, &cache_path, &cache_dir, target_size);
    }

    if is_img {
        return crate::utils::media::decode_image_thumbnail(path, &cache_path, target_size);
    }

    if is_vid {
        return crate::utils::media::extract_video_frame(
            path,
            &cache_path,
            &cache_dir,
            target_size,
        );
    }

    if is_pdf_file {
        return pdf_thumbnail(path, &cache_path, target_size);
    }

    if is_font_file {
        return crate::utils::media::font_thumbnail(path, &cache_path, target_size);
    }

    if is_audio {
        return crate::utils::media::audio_thumbnail(path, &cache_path, target_size);
    }

    if is_exe_file {
        return crate::utils::media::extract_exe_icon(path, &cache_path, target_size);
    }

    None
}
