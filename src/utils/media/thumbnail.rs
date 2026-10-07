use adw::gdk;
use gio::prelude::FileExt;
use std::fs;
use std::path::{Path, PathBuf};

use super::pdf::pdf_thumbnail;

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
/// required external tool (`ffmpeg`) is unavailable, or if I/O fails.
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
    let thumb_folder = match target_size {
        s if s > 512 => "xx-large",
        s if s > 256 => "x-large",
        s if s > 128 => "large",
        _ => "normal",
    };

    let cache_dir = dirs::cache_dir()?.join("thumbnails").join(thumb_folder);

    let uri = gio::File::for_path(path).uri();
    let hash = format!("{:x}", md5::compute(uri.as_bytes()));
    let cache_path = cache_dir.join(format!("{hash}.png"));

    Some((cache_dir, cache_path, hash))
}

/// Returns whether a cached thumbnail PNG is still valid for the given source file.
///
/// Compares the source's `mtime` against the thumbnail's own `mtime`. A thumbnail
/// is considered stale when the source was modified after the thumbnail was written,
/// or when the on-disk entry is zero bytes (partial write / crash residue).
///
/// # Arguments
///
/// * `cache_path`   - Path to the candidate `.png` thumbnail.
/// * `source_meta`  - [`fs::Metadata`] of the original source file.
fn thumbnail_is_valid(cache_path: &Path, source_meta: &fs::Metadata) -> bool {
    let cache_meta = match fs::metadata(cache_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let cache_mtime = cache_meta.modified().ok();
    let source_mtime = source_meta.modified().ok();

    // Source newer than cache → stale.
    if let (Some(ct), Some(st)) = (cache_mtime, source_mtime) {
        if st > ct {
            return false;
        }
    }

    // Guard against zero-byte truncation or partial writes.
    cache_meta.len() > 0
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

    let (is_img, is_vid) = if !is_pdf_file && !is_font_file && !is_exe_file && !is_audio {
        match ext.as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "heic" | "heif" | "bmp" | "tiff"
            | "tif" | "jxl" | "svg" | "ico" => (true, false),
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
        || (is_vid && thumb_types.videos)
        || (is_exe_file && thumb_types.executables)
        || (is_audio && thumb_types.audio);

    if !is_supported {
        return None;
    }

    let (cache_dir, cache_path, hash) = thumbnail_cache_path(path, target_size)?;

    let source_meta = std::fs::metadata(path).ok();

    let existing_thumbnail: Option<PathBuf> = if cache_path.exists() {
        Some(cache_path.clone())
    } else if let Some(base_cache) = dirs::cache_dir().map(|c| c.join("thumbnails")) {
        ["xx-large", "x-large", "large", "normal"]
            .iter()
            .map(|tier| base_cache.join(tier).join(format!("{hash}.png")))
            .find(|p| p.exists())
    } else {
        None
    };

    if let Some(existing) = existing_thumbnail {
        if let Ok(meta) = std::fs::metadata(&existing) {
            if meta.len() > 1024 * 1024 {
                let _ = std::fs::remove_file(&existing);
            } else {
                let is_valid = source_meta
                    .as_ref()
                    .map(|m| thumbnail_is_valid(&existing, m))
                    .unwrap_or(true);

                if is_valid {
                    if let Ok(bytes) = std::fs::read(&existing) {
                        if let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from(&bytes)) {
                            return Some(texture);
                        }
                    }
                }
                let _ = std::fs::remove_file(&existing);
            }
        }
    }

    let _ = std::fs::create_dir_all(&cache_dir);

    if is_img {
        return crate::utils::media::decode_image_thumbnail(
            path,
            &cache_path,
            &cache_dir,
            target_size,
        );
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
