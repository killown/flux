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
        size: if is_dir { 0 } else { size },
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::FileOptions;
    use zip::ZipWriter;

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Creates a temporary ZIP archive with a few entries.
    fn create_test_zip() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let zip_path = dir.path().join("test.zip");
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        // Explicit type for FileOptions
        let options: FileOptions<'_, ()> = FileOptions::default();

        zip.start_file("file1.txt", options).unwrap();
        zip.write_all(b"Hello").unwrap();

        zip.start_file("dir/file2.txt", options).unwrap();
        zip.write_all(b"World").unwrap();

        zip.start_file("dir/sub/file3.txt", options).unwrap();
        zip.write_all(b"Foo").unwrap();

        zip.finish().unwrap();
        (dir, zip_path)
    }

    /// Creates a temporary TAR archive (uncompressed) with entries.
    fn create_test_tar() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let tar_path = dir.path().join("test.tar");
        let mut builder = tar::Builder::new(File::create(&tar_path).unwrap());
        let mut header = tar::Header::new_gnu();
        header.set_path("file1.txt").unwrap();
        header.set_size(5);
        header.set_cksum();
        builder.append(&header, &b"Hello"[..]).unwrap();

        let mut header = tar::Header::new_gnu();
        header.set_path("dir/file2.txt").unwrap();
        header.set_size(5);
        header.set_cksum();
        builder.append(&header, &b"World"[..]).unwrap();

        let mut header = tar::Header::new_gnu();
        header.set_path("dir/sub/file3.txt").unwrap();
        header.set_size(3);
        header.set_cksum();
        builder.append(&header, &b"Foo"[..]).unwrap();

        builder.into_inner().unwrap();
        (dir, tar_path)
    }

    /// Creates a compressed TAR archive (tar.gz, tar.bz2, tar.xz) by compressing
    /// a pre-built tar buffer.
    fn create_compressed_tar(
        ext: &str,
        compress: impl Fn(&[u8]) -> Vec<u8>,
    ) -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let file_name = format!("test.tar.{}", ext);
        let path = dir.path().join(file_name);

        // Build uncompressed tar in memory.
        let mut tar_data = Vec::new();
        let mut builder = tar::Builder::new(&mut tar_data);
        let mut header = tar::Header::new_gnu();
        header.set_path("file1.txt").unwrap();
        header.set_size(5);
        header.set_cksum();
        builder.append(&header, &b"Hello"[..]).unwrap();

        let mut header = tar::Header::new_gnu();
        header.set_path("dir/file2.txt").unwrap();
        header.set_size(5);
        header.set_cksum();
        builder.append(&header, &b"World"[..]).unwrap();

        builder.into_inner().unwrap();

        // Compress and write.
        let compressed = compress(&tar_data);
        fs::write(&path, &compressed).unwrap();
        (dir, path)
    }

    // ── URI encoding/decoding ────────────────────────────────────────────────

    #[test]
    fn test_encode_decode_host() {
        let path = Path::new("/home/user/archive.zip");
        let encoded = encode_archive_host(path);
        assert_eq!(encoded, "%2Fhome%2Fuser%2Farchive.zip");
        let decoded = decode_archive_host(&encoded);
        assert_eq!(decoded, path);
    }

    #[test]
    fn test_encode_decode_host_with_special_chars() {
        let path = Path::new("/path/with%20space/archive.zip");
        let encoded = encode_archive_host(path);
        assert_eq!(encoded, "%2Fpath%2Fwith%2520space%2Farchive.zip");
        let decoded = decode_archive_host(&encoded);
        assert_eq!(decoded, path);
    }

    #[test]
    fn test_parse_archive_uri() {
        let uri = "/archive://%2Fhome%2Fuser%2Farchive.zip/dir/file.txt";
        let (archive_path, inner) = parse_archive_uri(uri).unwrap();
        assert_eq!(archive_path, Path::new("/home/user/archive.zip"));
        assert_eq!(inner, "dir/file.txt");
    }

    #[test]
    fn test_parse_archive_uri_root() {
        let uri = "/archive://%2Fhome%2Fuser%2Farchive.zip/";
        let (archive_path, inner) = parse_archive_uri(uri).unwrap();
        assert_eq!(archive_path, Path::new("/home/user/archive.zip"));
        assert_eq!(inner, "");
    }

    #[test]
    fn test_parse_archive_uri_no_inner() {
        let uri = "/archive://%2Fhome%2Fuser%2Farchive.zip";
        let (archive_path, inner) = parse_archive_uri(uri).unwrap();
        assert_eq!(archive_path, Path::new("/home/user/archive.zip"));
        assert_eq!(inner, "");
    }

    #[test]
    fn test_parse_archive_uri_invalid() {
        assert!(parse_archive_uri("archive://").is_none());
        assert!(parse_archive_uri("/archive://").is_none());
        assert!(parse_archive_uri("/archive:///").is_none());
    }

    #[test]
    fn test_build_archive_uri() {
        let archive = Path::new("/home/user/archive.zip");
        let uri = build_archive_uri(archive, "dir/file.txt");
        assert_eq!(
            uri.to_string_lossy(),
            "/archive://%2Fhome%2Fuser%2Farchive.zip/dir/file.txt"
        );
    }

    #[test]
    fn test_build_archive_uri_root() {
        let archive = Path::new("/home/user/archive.zip");
        let uri = build_archive_uri(archive, "");
        assert_eq!(
            uri.to_string_lossy(),
            "/archive://%2Fhome%2Fuser%2Farchive.zip/"
        );
    }

    // ── is_browsable_archive ─────────────────────────────────────────────────

    #[test]
    fn test_is_browsable_archive() {
        let cases = [
            ("archive.zip", true),
            ("archive.tar", true),
            ("archive.tar.gz", true),
            ("archive.tgz", true),
            ("archive.tar.bz2", true),
            ("archive.tbz2", true),
            ("archive.tar.xz", true),
            ("archive.txz", true),
            ("archive.7z", false),
            ("archive.rar", false),
            ("file.txt", false),
            ("archive.ZIP", true), // case-insensitive
            ("archive.TAR.GZ", true),
        ];
        for (name, expected) in cases {
            let path = Path::new(name);
            assert_eq!(is_browsable_archive(path), expected, "failed for {}", name);
        }
    }

    // ── list_archive_entries ──────────────────────────────────────────────────

    #[test]
    fn test_list_zip_root() {
        let (_dir, zip_path) = create_test_zip();
        let entries = list_archive_entries(&zip_path, "").unwrap();
        // Expect: "file1.txt", "dir" (synthesised directory)
        assert_eq!(entries.len(), 2);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"file1.txt"));
        assert!(names.contains(&"dir"));
        let dir_entry = entries.iter().find(|e| e.name == "dir").unwrap();
        assert!(dir_entry.is_dir);
        assert_eq!(dir_entry.size, 0);
        assert_eq!(dir_entry.inner_path, "dir");
    }

    #[test]
    fn test_list_zip_subdir() {
        let (_dir, zip_path) = create_test_zip();
        let entries = list_archive_entries(&zip_path, "dir").unwrap();
        // Expect: "file2.txt", "sub" (synthesised)
        assert_eq!(entries.len(), 2);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"file2.txt"));
        assert!(names.contains(&"sub"));
        let sub_entry = entries.iter().find(|e| e.name == "sub").unwrap();
        assert!(sub_entry.is_dir);
        assert_eq!(sub_entry.inner_path, "dir/sub");
    }

    #[test]
    fn test_list_zip_nested_subdir() {
        let (_dir, zip_path) = create_test_zip();
        let entries = list_archive_entries(&zip_path, "dir/sub").unwrap();
        // Expect: "file3.txt"
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "file3.txt");
        assert!(!entries[0].is_dir);
        assert_eq!(entries[0].size, 3);
        assert_eq!(entries[0].inner_path, "dir/sub/file3.txt");
    }

    #[test]
    fn test_list_tar_root() {
        let (_dir, tar_path) = create_test_tar();
        let entries = list_archive_entries(&tar_path, "").unwrap();
        // Similar to zip: "file1.txt", "dir" (synthesised from dir/file2.txt)
        assert_eq!(entries.len(), 2);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"file1.txt"));
        assert!(names.contains(&"dir"));
        let dir_entry = entries.iter().find(|e| e.name == "dir").unwrap();
        assert!(dir_entry.is_dir);
        assert_eq!(dir_entry.inner_path, "dir");
    }

    #[test]
    fn test_list_tar_gz() {
        let (_dir, path) = create_compressed_tar("gz", |data| {
            use flate2::write::GzEncoder;
            use flate2::Compression;
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data).unwrap();
            encoder.finish().unwrap()
        });
        let entries = list_archive_entries(&path, "").unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_list_tar_bz2() {
        let (_dir, path) = create_compressed_tar("bz2", |data| {
            use bzip2::write::BzEncoder;
            use bzip2::Compression;
            let mut encoder = BzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data).unwrap();
            encoder.finish().unwrap()
        });
        let entries = list_archive_entries(&path, "").unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_list_tar_xz() {
        let (_dir, path) = create_compressed_tar("xz", |data| {
            use xz2::write::XzEncoder;
            let mut encoder = XzEncoder::new(Vec::new(), 6);
            encoder.write_all(data).unwrap();
            encoder.finish().unwrap()
        });
        let entries = list_archive_entries(&path, "").unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_list_archive_empty_zip() {
        let dir = tempdir().unwrap();
        let zip_path = dir.path().join("empty.zip");
        File::create(&zip_path).unwrap(); // empty file, not a valid zip
        let result = list_archive_entries(&zip_path, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_archive_nonexistent() {
        let path = Path::new("/nonexistent/file.zip");
        let result = list_archive_entries(path, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_archive_unsupported_format() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.7z");
        fs::write(&path, b"dummy").unwrap();
        let result = list_archive_entries(&path, "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported archive format"));
    }

    // ── entries_to_load_contexts ─────────────────────────────────────────────

    #[test]
    fn test_entries_to_load_contexts() {
        let entries = vec![
            ArchiveEntry {
                name: "file.txt".to_string(),
                is_dir: false,
                size: 123,
                mtime: 456,
                inner_path: "file.txt".to_string(),
            },
            ArchiveEntry {
                name: "sub".to_string(),
                is_dir: true,
                size: 0,
                mtime: 0,
                inner_path: "sub".to_string(),
            },
        ];
        let archive_path = Path::new("/home/user/archive.zip");
        let contexts = entries_to_load_contexts(&entries, archive_path, true);
        assert_eq!(contexts.len(), 2);
        let c1 = &contexts[0];
        assert_eq!(c1.display_name, "file.txt");
        assert!(!c1.is_dir);
        assert_eq!(c1.size, 123);
        assert_eq!(c1.mtime, 456);
        assert!(c1.expand_labels);
        assert_eq!(c1.target_path, build_archive_uri(archive_path, "file.txt"));
        let c2 = &contexts[1];
        assert!(c2.is_dir);
        assert_eq!(c2.target_path, build_archive_uri(archive_path, "sub"));
    }

    // ── extract_entry_to_tempfile ────────────────────────────────────────────

    #[test]
    fn test_extract_zip_entry() {
        let (_dir, zip_path) = create_test_zip();
        let tmp = extract_entry_to_tempfile(&zip_path, "file1.txt").unwrap();
        let content = fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "Hello");
        // Keep the temp file alive until end of test.
        tmp.keep().ok();
    }

    #[test]
    fn test_extract_zip_entry_in_subdir() {
        let (_dir, zip_path) = create_test_zip();
        let tmp = extract_entry_to_tempfile(&zip_path, "dir/file2.txt").unwrap();
        let content = fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "World");
        tmp.keep().ok();
    }

    #[test]
    fn test_extract_tar_entry() {
        let (_dir, tar_path) = create_test_tar();
        let tmp = extract_entry_to_tempfile(&tar_path, "file1.txt").unwrap();
        let content = fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "Hello");
        tmp.keep().ok();
    }

    #[test]
    fn test_extract_tar_gz_entry() {
        let (_dir, path) = create_compressed_tar("gz", |data| {
            use flate2::write::GzEncoder;
            use flate2::Compression;
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data).unwrap();
            encoder.finish().unwrap()
        });
        let tmp = extract_entry_to_tempfile(&path, "file1.txt").unwrap();
        let content = fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "Hello");
        tmp.keep().ok();
    }

    #[test]
    fn test_extract_entry_not_found() {
        let (_dir, zip_path) = create_test_zip();
        let result = extract_entry_to_tempfile(&zip_path, "nonexistent.txt");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Entry not found"));
    }

    #[test]
    fn test_extract_from_unsupported_archive() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.7z");
        fs::write(&path, b"dummy").unwrap();
        let result = extract_entry_to_tempfile(&path, "file");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported archive format"));
    }
}
