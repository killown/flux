//! The `/archive://` URI scheme: encoding, parsing and construction.

use crate::services::archive::extract_entry_to_tempfile;
use std::path::{Path, PathBuf};

// ─── URI scheme ───────────────────────────────────────────────────────────────

/// URI scheme prefix used throughout the app to identify archive virtual paths.
///
/// The leading `/` makes `PathBuf::from(uri)` treat it as an absolute path,
/// preventing the OS from prepending the CWD when stored in a `PathBuf` field.
pub const ARCHIVE_URI: &str = "/archive://";

/// Encodes an absolute archive path into the host component of an archive URI.
#[inline]
pub fn encode_archive_host(archive_path: &Path) -> String {
    archive_path
        .to_string_lossy()
        .replace('%', "%25")
        .replace('/', "%2F")
}

/// Decodes the host component of an archive URI back to an absolute filesystem path.
///
/// Performs a single-pass percent decode. Every `%XX` sequence (uppercase or
/// lowercase hex) is decoded exactly once - there is no ordering hazard because
/// the decode buffer is never re-scanned. This makes `decode(encode(x)) == x`
/// hold for any `Path`, including paths containing literal `%2F`, `%25`, or
/// trailing `%` characters.
#[inline]
pub fn decode_archive_host(host: &str) -> PathBuf {
    let bytes = host.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }

    // Lossy conversion mirrors the encoder, which uses `to_string_lossy()`.
    PathBuf::from(String::from_utf8_lossy(&out).into_owned())
}

/// Extracts an archive entry to a persistent preview cache for media and
/// font preview.
///
/// Uses `~/.cache/flux/previews/` instead of the flux scratch dir, because
/// font previews register the extracted file with Fontconfig, and Fontconfig
/// (via Cairo/Pango) reads the file lazily when glyphs are rasterized. If
/// the file is deleted before rasterization (which is what happens with the
/// scratch dir, since it is cleared on navigate-away), Cairo reports
/// "file not found" and the preview renders as tofu.
///
/// The cache dir is not cleared on navigation. Files accumulate until the
/// OS cache-cleanup policy or the user removes them, the total is bounded
/// by the number of archives previewed per session.
pub fn extract_archive_entry_for_playback(
    archive_path: &std::path::Path,
    inner_path: &str,
) -> Option<std::path::PathBuf> {
    let cache_dir = dirs::cache_dir()?.join("flux/previews");
    std::fs::create_dir_all(&cache_dir).ok()?;

    let original_ext = std::path::Path::new(inner_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // Key the cache filename on (archive path, inner path, mtime) so repeated
    // previews of the same entry within a session (thumbnail + panel) reuse
    // the same file instead of extracting twice.
    let archive_mtime = std::fs::metadata(archive_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let key = format!(
        "{}:{}:{}",
        archive_path.display(),
        inner_path,
        archive_mtime,
    );
    let hash = format!("{:x}", md5::compute(key.as_bytes()));

    let visible_name = if original_ext.is_empty() {
        format!("preview-{}", hash)
    } else {
        format!("preview-{}.{}", hash, original_ext)
    };

    let visible_path = cache_dir.join(&visible_name);
    if visible_path.exists() {
        return Some(visible_path);
    }

    let temp_file = extract_entry_to_tempfile(archive_path, inner_path, None).ok()?;

    // `/tmp` is often tmpfs while `~/.cache` lives on disk, `fs::rename`
    // returns EXDEV across filesystems. Fall back to copy + unlink so the
    // persistent preview cache is actually populated.
    if std::fs::rename(temp_file.path(), &visible_path).is_err() {
        std::fs::copy(temp_file.path(), &visible_path).ok()?;
    }

    let _ = temp_file.keep();
    Some(visible_path)
}

/// Splits an `archive://` URI into `(archive_path_on_disk, inner_prefix)`.
///
/// `inner_prefix` is the path inside the archive being browsed (`""` = root).
#[allow(dead_code)]
pub fn parse_archive_uri(uri: &str) -> Option<(PathBuf, String)> {
    crate::hit!("parse_archive_uri");

    // Strip all possible URI prefixes that GTK / GIO / PathBuf might produce.
    let mut rest = uri
        .strip_prefix(ARCHIVE_URI)
        .or_else(|| uri.strip_prefix("archive://"))
        .or_else(|| uri.strip_prefix("/archive:/"))
        .or_else(|| uri.strip_prefix("archive:/"))?;

    // CRITICAL: Strip any leftover leading slashes so rest doesn't start with '/'
    rest = rest.trim_start_matches('/');

    let (host, inner) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx + 1..]),
        None => (rest, ""),
    };

    if host.is_empty() {
        return None;
    }

    // WARNING: reject any inner path that could escape the archive root.
    // Downstream callers join `inner` onto filesystem paths or use it as an
    // archive entry name, and a `..` sequence would either write outside the
    // intended directory or produce an entry that normalises to something
    // unexpected in the resulting archive.
    for comp in Path::new(inner).components() {
        if matches!(
            comp,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        ) {
            return None;
        }
    }

    Some((decode_archive_host(host), inner.to_owned()))
}

/// Returns an archive entry's thumbnail. Zips check the in-zip cache first
/// and queue the generated PNG for later. Other formats just generate.
pub fn get_archive_thumbnail(path: &std::path::Path) -> Option<gtk::gdk::Texture> {
    let path_str = path.to_string_lossy();
    let (archive_path, inner_path) = parse_archive_uri(&path_str)?;
    let is_zip = archive_path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
        && crate::utils::load_config().ui.archive_thumbnail_cache;

    if is_zip {
        if let Some(tex) =
            crate::services::archive::read_cached_thumbnail(&archive_path, &inner_path, None)
        {
            return Some(tex);
        }
    }

    // Persistent cache: `font_thumbnail` registers this path with fontconfig
    // and Cairo reads it lazily when glyphs are rasterized. A temp file that
    // `get_archive_thumbnail` deletes on return would poison fontconfig and
    // render tofu on the next preview.
    let extracted_path = extract_archive_entry_for_playback(&archive_path, &inner_path)?;
    let tex = crate::utils::media::generate_thumbnail_sync(&extracted_path)?;

    if is_zip {
        use gtk::gdk::prelude::TextureExt;
        let png = tex.save_to_png_bytes();
        if !png.is_empty() {
            crate::services::archive::thumb_cache::enqueue(
                archive_path,
                inner_path,
                png.to_vec(),
                None,
            );
        }
    }

    Some(tex)
}

/// Constructs an `archive://` URI `PathBuf` for a given archive file and inner path.
#[inline]
pub fn build_archive_uri(archive_path: &Path, inner_path: &str) -> PathBuf {
    crate::hit!("build_archive_uri");
    let host = encode_archive_host(archive_path);
    let uri = if inner_path.is_empty() {
        format!("{}{}/", ARCHIVE_URI, host)
    } else {
        format!("{}{}/{}", ARCHIVE_URI, host, inner_path)
    };
    PathBuf::from(uri)
}
