//! Virtual read-only filesystem layer for browsing compressed archives.
//!
//! Implements a URI scheme `archive://<encoded_host>/<inner_path>` where the
//! host component is the percent-encoded absolute path of the archive on disk.
//! This mirrors the pattern established by `trash:///` and `recent:///` so that
//! the rest of the app needs no special casing beyond recognising the scheme prefix.
//!
//! # Supported formats
//! - ZIP  (`.zip`)
//! - Gzip-compressed tar  (`.tar.gz`, `.tgz`)
//! - Bzip2-compressed tar (`.tar.bz2`, `.tbz2`)
//! - XZ-compressed tar    (`.tar.xz`, `.txz`)
//! - Uncompressed tar     (`.tar`)
//!
//! 7-Zip (`.7z`) requires a native C library and is deliberately deferred to a
//! future integration, attempting to enter a `.7z` file falls back to the
//! default `xdg-open` handler.

use std::collections::HashMap;
use std::io::{BufReader, Read, Seek};
use std::path::{Path, PathBuf};

use crate::model::FileLoadContext;

/// URI scheme prefix used throughout the app to identify archive virtual paths.
///
/// The leading `/` makes `PathBuf::from(uri)` treat it as an absolute path,
/// preventing the OS from prepending the current working directory when the
/// string is stored in a `PathBuf` field and later retrieved via `to_string_lossy`.
pub const ARCHIVE_URI: &str = "/archive://";

/// Encodes an absolute archive path into the host component of an archive URI.
///
/// Uses percent-encoding for `/` so the result fits in a single URI authority
/// segment. This is intentionally simple and not RFC 3986 general-purpose.
///
/// # Arguments
/// * `archive_path` - Absolute path of the archive file on disk.
#[inline]
pub fn encode_archive_host(archive_path: &Path) -> String {
    archive_path
        .to_string_lossy()
        .replace('%', "%25")
        .replace('/', "%2F")
}

/// Decodes the host component of an archive URI back to an absolute filesystem path.
///
/// # Arguments
/// * `host` - The host segment extracted from an `archive://` URI.
#[inline]
pub fn decode_archive_host(host: &str) -> PathBuf {
    PathBuf::from(host.replace("%2F", "/").replace("%25", "%"))
}

/// Splits an `archive://` URI into `(archive_path_on_disk, inner_prefix)`.
///
/// `inner_prefix` is the path inside the archive being browsed (empty string = root).
///
/// # Arguments
/// * `uri` - A string starting with `archive://`.
///
/// # Returns
/// `None` if `uri` does not match the expected scheme or host is absent.
pub fn parse_archive_uri(uri: &str) -> Option<(PathBuf, String)> {
    let rest = uri.strip_prefix(ARCHIVE_URI)?;
    // Split on the first '/' that follows the (encoded) host
    let (host, inner) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx + 1..]),
        None => (rest, ""),
    };
    if host.is_empty() {
        return None;
    }
    Some((decode_archive_host(host), inner.to_owned()))
}

/// Constructs an `archive://` URI for a given archive file and inner path.
///
/// # Arguments
/// * `archive_path` - Absolute path of the archive on disk.
/// * `inner_path`   - Path inside the archive (use `""` for the root listing).
#[inline]
pub fn build_archive_uri(archive_path: &Path, inner_path: &str) -> PathBuf {
    let host = encode_archive_host(archive_path);
    let uri = if inner_path.is_empty() {
        format!("{}{}/", ARCHIVE_URI, host)
    } else {
        format!("{}{}/{}", ARCHIVE_URI, host, inner_path)
    };
    PathBuf::from(uri)
}

/// Detects whether a file extension indicates a supported browsable archive.
///
/// Returns `true` only for formats that `list_archive_entries` can handle.
///
/// # Arguments
/// * `path` - Any path whose extension will be inspected.
pub fn is_browsable_archive(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    name.ends_with(".zip")
        || name.ends_with(".tar")
        || name.ends_with(".tar.gz")
        || name.ends_with(".tgz")
        || name.ends_with(".tar.bz2")
        || name.ends_with(".tbz2")
        || name.ends_with(".tar.xz")
        || name.ends_with(".txz")
}

/// Describes a single entry within an archive as seen from a specific directory level.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    /// Display name (filename component only).
    pub name: String,
    /// Whether this entry is a directory (real or synthesised from a longer path).
    pub is_dir: bool,
    /// Uncompressed size in bytes. `0` for synthesised directory nodes.
    pub size: u64,
    /// Last-modified unix timestamp. `0` when unavailable.
    pub mtime: i64,
    /// Full inner path relative to the archive root, used to build child URIs.
    pub inner_path: String,
}

/// Lists the immediate children of `prefix` inside the archive at `archive_path`.
///
/// This performs a single-pass scan of all entries and synthesises virtual
/// directory nodes for any intermediate path components that are not explicitly
/// stored in the archive (common in ZIP files). The result mirrors what
/// `gio::File::enumerate_children` returns for a real directory.
///
/// # Arguments
/// * `archive_path` - Absolute path of the archive on disk.
/// * `prefix`       - Inner path to treat as the current directory root.
///
/// # Errors
/// Returns an `Err` string suitable for a toast notification on I/O or
/// format-detection failure.
pub fn list_archive_entries(
    archive_path: &Path,
    prefix: &str,
) -> Result<Vec<ArchiveEntry>, String> {
    let name_lc = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if name_lc.ends_with(".zip") {
        list_zip(archive_path, prefix)
    } else if name_lc.ends_with(".tar.gz") || name_lc.ends_with(".tgz") {
        list_tar_gz(archive_path, prefix)
    } else if name_lc.ends_with(".tar.bz2") || name_lc.ends_with(".tbz2") {
        list_tar_bz2(archive_path, prefix)
    } else if name_lc.ends_with(".tar.xz") || name_lc.ends_with(".txz") {
        list_tar_xz(archive_path, prefix)
    } else if name_lc.ends_with(".tar") {
        list_tar_plain(archive_path, prefix)
    } else {
        Err(format!(
            "Unsupported archive format: {}",
            archive_path.display()
        ))
    }
}

/// Converts a list of [`ArchiveEntry`] items into [`FileLoadContext`] records
/// suitable for direct insertion into the file grid model.
///
/// Sorting and thumbnail eligibility are handled by the caller (`load_archive`
/// in `loader.rs`) following the same pipeline used for real directories.
///
/// # Arguments
/// * `entries`      - Flat slice of archive entries at the current level.
/// * `archive_path` - Needed to build child `archive://` URIs.
/// * `icon_size`    - Passed through into each `FileLoadContext`.
/// * `expand_labels`- Passed through into each `FileLoadContext`.
pub fn entries_to_load_contexts(
    entries: &[ArchiveEntry],
    archive_path: &Path,
    expand_labels: bool,
) -> Vec<FileLoadContext> {
    entries
        .iter()
        .map(|e| {
            let target_path = build_archive_uri(archive_path, &e.inner_path);
            FileLoadContext {
                display_name: e.name.clone(),
                sort_name: e.name.to_lowercase(),
                target_path,
                size: e.size,
                mtime: e.mtime,
                is_dir: e.is_dir,
                thumbnail_path: None,
                is_foreign_owner: false,
                expand_labels,
                custom_icon: None,
            }
        })
        .collect()
}

/// Extracts a single file entry from an archive into a temporary file and returns
/// its path so the host OS can open it with the appropriate application.
///
/// The returned [`tempfile::NamedTempFile`] must be kept alive for as long as the
/// spawned viewer process may need it. Callers should call `.keep()` to persist it
/// until the next application restart, after which the OS will clean up `/tmp`.
///
/// The temporary file is created with the original filename as the suffix so that
/// `xdg-open` can identify the MIME type by extension.
///
/// # Arguments
/// * `archive_path` - Real on-disk path of the archive.
/// * `inner_path`   - Entry path relative to the archive root (no leading `/`).
///
/// # Errors
/// Returns an `Err` string on I/O failure, missing entry, or unsupported format.
#[allow(dead_code)]
pub fn extract_entry_to_tempfile(
    archive_path: &Path,
    inner_path: &str,
) -> Result<tempfile::NamedTempFile, String> {
    let name_lc = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let suffix = std::path::Path::new(inner_path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| format!(".{n}"))
        .unwrap_or_default();

    let tmp = tempfile::Builder::new()
        .suffix(&suffix)
        .tempfile()
        .map_err(|e| format!("Cannot create temp file: {e}"))?;

    if name_lc.ends_with(".zip") {
        extract_zip_entry(archive_path, inner_path, tmp)
    } else if name_lc.ends_with(".tar.gz") || name_lc.ends_with(".tgz") {
        extract_tar_entry(
            flate2::read::GzDecoder::new(open_buffered(archive_path)?),
            inner_path,
            tmp,
        )
    } else if name_lc.ends_with(".tar.bz2") || name_lc.ends_with(".tbz2") {
        extract_tar_entry(
            bzip2::read::BzDecoder::new(open_buffered(archive_path)?),
            inner_path,
            tmp,
        )
    } else if name_lc.ends_with(".tar.xz") || name_lc.ends_with(".txz") {
        extract_tar_entry(
            xz2::read::XzDecoder::new(open_buffered(archive_path)?),
            inner_path,
            tmp,
        )
    } else if name_lc.ends_with(".tar") {
        extract_tar_entry(open_buffered(archive_path)?, inner_path, tmp)
    } else {
        Err(format!(
            "Unsupported archive format: {}",
            archive_path.display()
        ))
    }
}

fn extract_zip_entry(
    archive_path: &Path,
    inner_path: &str,
    mut tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, String> {
    use std::io::Write;

    let file =
        std::fs::File::open(archive_path).map_err(|e| format!("Cannot open archive: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Invalid ZIP: {e}"))?;

    let mut entry = zip
        .by_name(inner_path)
        .map_err(|_| format!("Entry not found in ZIP: {inner_path}"))?;

    std::io::copy(&mut entry, &mut tmp).map_err(|e| format!("ZIP extract error: {e}"))?;
    tmp.flush().map_err(|e| format!("Flush error: {e}"))?;
    Ok(tmp)
}

fn extract_tar_entry<R: Read>(
    reader: R,
    inner_path: &str,
    mut tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, String> {
    use std::io::Write;

    let mut archive = tar::Archive::new(reader);

    for entry in archive
        .entries()
        .map_err(|e| format!("TAR read error: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("TAR entry error: {e}"))?;
        let path = entry
            .path()
            .map_err(|e| format!("TAR path error: {e}"))?
            .to_string_lossy()
            .replace('\\', "/");
        let path = path.trim_end_matches('/');

        if path == inner_path {
            std::io::copy(&mut entry, &mut tmp).map_err(|e| format!("TAR extract error: {e}"))?;
            tmp.flush().map_err(|e| format!("Flush error: {e}"))?;
            return Ok(tmp);
        }
    }

    Err(format!("Entry not found in TAR: {inner_path}"))
}

// ─── Format-specific implementations ─────────────────────────────────────────

fn list_zip(archive_path: &Path, prefix: &str) -> Result<Vec<ArchiveEntry>, String> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| format!("Cannot open archive: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Invalid ZIP: {e}"))?;

    let mut seen: HashMap<String, ArchiveEntry> = HashMap::new();

    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|e| format!("ZIP read error: {e}"))?;

        let raw_name = entry.name().replace('\\', "/");
        let raw_name = raw_name.trim_end_matches('/');

        let is_entry_dir = entry.is_dir();
        let size = entry.size();
        let mtime = zip_mtime(&entry);

        collect_entry(&mut seen, raw_name, is_entry_dir, size, mtime, prefix);
    }

    Ok(seen.into_values().collect())
}

fn list_tar_gz(archive_path: &Path, prefix: &str) -> Result<Vec<ArchiveEntry>, String> {
    let file = open_buffered(archive_path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    list_tar_reader(decoder, prefix)
}

fn list_tar_bz2(archive_path: &Path, prefix: &str) -> Result<Vec<ArchiveEntry>, String> {
    let file = open_buffered(archive_path)?;
    let decoder = bzip2::read::BzDecoder::new(file);
    list_tar_reader(decoder, prefix)
}

fn list_tar_xz(archive_path: &Path, prefix: &str) -> Result<Vec<ArchiveEntry>, String> {
    let file = open_buffered(archive_path)?;
    let decoder = xz2::read::XzDecoder::new(file);
    list_tar_reader(decoder, prefix)
}

fn list_tar_plain(archive_path: &Path, prefix: &str) -> Result<Vec<ArchiveEntry>, String> {
    let file = open_buffered(archive_path)?;
    list_tar_reader(file, prefix)
}

fn list_tar_reader<R: Read>(reader: R, prefix: &str) -> Result<Vec<ArchiveEntry>, String> {
    let mut archive = tar::Archive::new(reader);
    let mut seen: HashMap<String, ArchiveEntry> = HashMap::new();

    for entry in archive
        .entries()
        .map_err(|e| format!("TAR read error: {e}"))?
    {
        let entry = entry.map_err(|e| format!("TAR entry error: {e}"))?;
        let header = entry.header();

        let raw_name = entry
            .path()
            .map_err(|e| format!("TAR path error: {e}"))?
            .to_string_lossy()
            .replace('\\', "/");
        let raw_name = raw_name.trim_end_matches('/');

        let is_entry_dir = header.entry_type().is_dir();
        let size = header.size().unwrap_or(0);
        let mtime = header.mtime().unwrap_or(0) as i64;

        collect_entry(&mut seen, raw_name, is_entry_dir, size, mtime, prefix);
    }

    Ok(seen.into_values().collect())
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

/// Resolves one archive entry path against the current `prefix` and inserts
/// either the direct child or a synthesised intermediate directory node into `seen`.
fn collect_entry(
    seen: &mut HashMap<String, ArchiveEntry>,
    raw_name: &str,
    is_entry_dir: bool,
    size: u64,
    mtime: i64,
    prefix: &str,
) {
    let relative = if prefix.is_empty() {
        raw_name.to_owned()
    } else {
        let pfx = prefix.trim_end_matches('/');
        match raw_name.strip_prefix(&format!("{pfx}/")) {
            Some(r) if !r.is_empty() => r.to_owned(),
            // Entry does not live under this prefix (or IS the prefix dir itself)
            _ => return,
        }
    };

    // Determine the immediate child name under prefix
    let (child_name, is_dir, child_inner) = match relative.find('/') {
        Some(slash_pos) => {
            // Intermediate directory node
            let dir_name = &relative[..slash_pos];
            let inner = if prefix.is_empty() {
                dir_name.to_owned()
            } else {
                format!("{}/{}", prefix.trim_end_matches('/'), dir_name)
            };
            (dir_name.to_owned(), true, inner)
        }
        None => {
            // Direct child
            let inner = if prefix.is_empty() {
                relative.clone()
            } else {
                format!("{}/{}", prefix.trim_end_matches('/'), relative)
            };
            (relative, is_entry_dir, inner)
        }
    };

    seen.entry(child_name.clone()).or_insert(ArchiveEntry {
        name: child_name,
        is_dir,
        size,
        mtime,
        inner_path: child_inner,
    });
}

#[inline]
fn open_buffered(path: &Path) -> Result<BufReader<std::fs::File>, String> {
    std::fs::File::open(path)
        .map(BufReader::new)
        .map_err(|e| format!("Cannot open archive: {e}"))
}

/// Extracts a last-modified unix timestamp from a ZIP entry's `last_modified` field.
fn zip_mtime<R: Read + Seek>(entry: &zip::read::ZipFile<'_, R>) -> i64 {
    entry
        .last_modified()
        .map(|dt| {
            // ZipDateTime does not implement Into<i64> directly, approximate via components.
            // This is display-only - no need for full calendar arithmetic.
            let y = dt.year() as i64;
            let m = dt.month() as i64;
            let d = dt.day() as i64;
            let h = dt.hour() as i64;
            let min = dt.minute() as i64;
            let s = dt.second() as i64;
            // Epoch approximation: good enough for sort ordering in the file grid.
            (y - 1970) * 31_557_600 + m * 2_629_800 + d * 86_400 + h * 3_600 + min * 60 + s
        })
        .unwrap_or(0)
}
