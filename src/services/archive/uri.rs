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

/// Extracts an archive entry to a temporary path and generates its thumbnail.
pub fn get_archive_thumbnail(path: &std::path::Path) -> Option<gtk::gdk::Texture> {
    let path_str = path.to_string_lossy();
    let (archive_path, inner_path) = parse_archive_uri(&path_str)?;
    let extracted_path = extract_archive_entry_to_temp(&archive_path, &inner_path)?;

    let texture = crate::utils::media::generate_thumbnail_sync(&extracted_path);
    let _ = std::fs::remove_file(&extracted_path);

    texture
}

/// Extracts an archive entry to a temporary file path.
pub fn extract_archive_entry_to_temp(
    archive_path: &std::path::Path,
    inner_path: &str,
) -> Option<std::path::PathBuf> {
    let temp_file = extract_entry_to_tempfile(archive_path, inner_path, None).ok()?;
    let (_, path) = temp_file.keep().ok()?;
    Some(path)
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
