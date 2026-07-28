//! Virtual read-only filesystem layer for browsing compressed archives.
//!
//! Implements a URI scheme `/archive://<encoded_host>/<inner_path>` where the
//! host component is the percent-encoded absolute path of the archive on disk.
//!
//! # Supported formats
//! | Extension                          | Backend          | Password |
//! |------------------------------------|------------------|----------|
//! | `.zip`                             | `zip`            | ✓        |
//! | `.tar`, `.tar.gz`, `.tgz`          | `tar` + `flate2` | ✗        |
//! | `.tar.bz2`, `.tbz2`                | `tar` + `bzip2`  | ✗        |
//! | `.tar.xz`, `.txz`                  | `tar` + `xz2`    | ✗        |
//! | `.tar.zst`, `.tzst`                | `tar` + `zstd`   | ✗        |
//! | `.tar.lz4`                         | `tar` + `lz4`    | ✗        |
//! | `.7z`                              | `sevenz-rust2`   | ✓        |
//! | `.gz` (standalone)                 | `flate2`         | ✗        |
//! | `.bz2` (standalone)                | `bzip2`          | ✗        |
//! | `.xz` (standalone)                 | `xz2`            | ✗        |
//! | `.zst` / `.zstd` (standalone)      | `zstd`           | ✗        |
//! | `.lz4` (standalone)                | `lz4_flex`       | ✗        |

use std::collections::HashMap;
use std::io::{BufReader, Read, Seek, Write};
use std::path::{Path, PathBuf};

use crate::model::FileLoadContext;
use sevenz_rust2::{ArchiveReader, Password};

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
#[inline]
pub fn decode_archive_host(host: &str) -> PathBuf {
    PathBuf::from(host.replace("%2F", "/").replace("%25", "%"))
}

/// Splits an `archive://` URI into `(archive_path_on_disk, inner_prefix)`.
///
/// `inner_prefix` is the path inside the archive being browsed (`""` = root).
#[allow(dead_code)]
pub fn parse_archive_uri(uri: &str) -> Option<(PathBuf, String)> {
    // Strip either the absolute form (/archive://) or bare form (archive://)
    let rest = uri
        .strip_prefix(ARCHIVE_URI)
        .or_else(|| uri.strip_prefix("archive://"))?;

    let (host, inner) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx + 1..]),
        None => (rest, ""),
    };
    if host.is_empty() {
        return None;
    }
    Some((decode_archive_host(host), inner.to_owned()))
}

/// Constructs an `archive://` URI `PathBuf` for a given archive file and inner path.
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

// ─── Format detection ─────────────────────────────────────────────────────────

/// Returns `true` for archives that `list_archive_entries` can browse.
#[allow(dead_code)]
pub fn is_browsable_archive(path: &Path) -> bool {
    matches_extension(
        path,
        &[
            "zip", "tar", "tar.gz", "tgz", "tar.bz2", "tbz2", "tar.xz", "txz", "tar.zst", "tzst",
            "tar.lz4", "7z", "rar", "gz", "bz2", "xz", "zst", "zstd", "lz4",
        ],
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

// ─── Error type ───────────────────────────────────────────────────────────────

/// Error returned by listing/extraction operations.
#[derive(Debug)]
pub enum ArchiveError {
    /// The archive is encrypted and requires a password.
    PasswordRequired,
    /// The supplied password was rejected.
    WrongPassword,
    /// Any other I/O or format error, contains a human-readable message.
    Other(String),
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::PasswordRequired => write!(f, "Archive is password-protected"),
            ArchiveError::WrongPassword => write!(f, "Incorrect password"),
            ArchiveError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<String> for ArchiveError {
    fn from(s: String) -> Self {
        ArchiveError::Other(s)
    }
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Describes a single entry within an archive as seen from a specific directory level.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub is_dir: bool,
    /// Uncompressed size in bytes, `0` for synthesised directory nodes.
    pub size: u64,
    /// Last-modified unix timestamp, `0` when unavailable.
    pub mtime: i64,
    /// Full inner path relative to the archive root, used to build child URIs.
    pub inner_path: String,
}

/// Lists immediate children of `prefix` inside the archive at `archive_path`.
///
/// Pass `password` as `Some(pwd)` for encrypted archives.
///
/// # Errors
/// Returns [`ArchiveError::PasswordRequired`] when the archive is encrypted
/// and no password was supplied, allowing the caller to prompt the user and retry.
#[allow(dead_code)]
pub fn list_archive_entries(
    archive_path: &Path,
    prefix: &str,
    password: Option<&str>,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let name_lc = lc_name(archive_path);

    if name_lc.ends_with(".zip") {
        list_zip(archive_path, prefix, password)
    } else if name_lc.ends_with(".7z") {
        list_7z(archive_path, prefix, password)
    } else if name_lc.ends_with(".tar.gz") || name_lc.ends_with(".tgz") {
        list_tar(
            flate2::read::GzDecoder::new(open_buf(archive_path)?),
            prefix,
        )
    } else if name_lc.ends_with(".tar.bz2") || name_lc.ends_with(".tbz2") {
        list_tar(bzip2::read::BzDecoder::new(open_buf(archive_path)?), prefix)
    } else if name_lc.ends_with(".tar.xz") || name_lc.ends_with(".txz") {
        list_tar(xz2::read::XzDecoder::new(open_buf(archive_path)?), prefix)
    } else if name_lc.ends_with(".tar.zst") || name_lc.ends_with(".tzst") {
        let dec = zstd::stream::read::Decoder::new(open_buf(archive_path)?)
            .map_err(|e| ArchiveError::Other(format!("zstd open: {e}")))?;
        list_tar(dec, prefix)
    } else if name_lc.ends_with(".tar.lz4") {
        let dec = lz4_flex::frame::FrameDecoder::new(open_buf(archive_path)?);
        list_tar(dec, prefix)
    } else if name_lc.ends_with(".tar") {
        list_tar(open_buf(archive_path)?, prefix)
    } else if name_lc.ends_with(".gz") {
        // Standalone gzip - single-file pseudo-dir
        single_file_listing(archive_path, strip_ext(&name_lc, ".gz"))
    } else if name_lc.ends_with(".bz2") {
        single_file_listing(archive_path, strip_ext(&name_lc, ".bz2"))
    } else if name_lc.ends_with(".xz") {
        single_file_listing(archive_path, strip_ext(&name_lc, ".xz"))
    } else if name_lc.ends_with(".zstd") {
        single_file_listing(archive_path, strip_ext(&name_lc, ".zstd"))
    } else if name_lc.ends_with(".zst") {
        single_file_listing(archive_path, strip_ext(&name_lc, ".zst"))
    } else if name_lc.ends_with(".lz4") {
        single_file_listing(archive_path, strip_ext(&name_lc, ".lz4"))
    } else if name_lc.ends_with(".rar") {
        list_rar(archive_path, prefix, password)
    } else {
        Err(ArchiveError::Other(format!(
            "Unsupported format: {}",
            archive_path.display()
        )))
    }
}

/// Converts [`ArchiveEntry`] items into [`FileLoadContext`] records for the grid model.
#[allow(dead_code)]
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

// ─── Extraction ───────────────────────────────────────────────────────────────

/// Extracts a single file entry to a [`tempfile::NamedTempFile`] for `xdg-open`.
///
/// Callers should call `.keep()` on the returned file to persist it until the
/// next app restart, after which the OS cleans `/tmp`.
#[allow(dead_code)]
pub fn extract_entry_to_tempfile(
    archive_path: &Path,
    inner_path: &str,
    password: Option<&str>,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    let suffix = Path::new(inner_path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| format!(".{n}"))
        .unwrap_or_default();

    let tmp = tempfile::Builder::new()
        .suffix(&suffix)
        .tempfile()
        .map_err(|e| ArchiveError::Other(format!("temp file: {e}")))?;

    let name_lc = lc_name(archive_path);

    if name_lc.ends_with(".zip") {
        extract_zip(archive_path, inner_path, password, tmp)
    } else if name_lc.ends_with(".7z") {
        extract_7z(archive_path, inner_path, password, tmp)
    } else if name_lc.ends_with(".tar.gz") || name_lc.ends_with(".tgz") {
        extract_tar(
            flate2::read::GzDecoder::new(open_buf(archive_path)?),
            inner_path,
            tmp,
        )
    } else if name_lc.ends_with(".tar.bz2") || name_lc.ends_with(".tbz2") {
        extract_tar(
            bzip2::read::BzDecoder::new(open_buf(archive_path)?),
            inner_path,
            tmp,
        )
    } else if name_lc.ends_with(".tar.xz") || name_lc.ends_with(".txz") {
        extract_tar(
            xz2::read::XzDecoder::new(open_buf(archive_path)?),
            inner_path,
            tmp,
        )
    } else if name_lc.ends_with(".tar.zst") || name_lc.ends_with(".tzst") {
        let dec = zstd::stream::read::Decoder::new(open_buf(archive_path)?)
            .map_err(|e| ArchiveError::Other(format!("zstd: {e}")))?;
        extract_tar(dec, inner_path, tmp)
    } else if name_lc.ends_with(".tar.lz4") {
        extract_tar(
            lz4_flex::frame::FrameDecoder::new(open_buf(archive_path)?),
            inner_path,
            tmp,
        )
    } else if name_lc.ends_with(".tar") {
        extract_tar(open_buf(archive_path)?, inner_path, tmp)
    } else if name_lc.ends_with(".gz") {
        extract_single(flate2::read::GzDecoder::new(open_buf(archive_path)?), tmp)
    } else if name_lc.ends_with(".bz2") {
        extract_single(bzip2::read::BzDecoder::new(open_buf(archive_path)?), tmp)
    } else if name_lc.ends_with(".xz") {
        extract_single(xz2::read::XzDecoder::new(open_buf(archive_path)?), tmp)
    } else if name_lc.ends_with(".zstd") || name_lc.ends_with(".zst") {
        let dec = zstd::stream::read::Decoder::new(open_buf(archive_path)?)
            .map_err(|e| ArchiveError::Other(format!("zstd: {e}")))?;
        extract_single(dec, tmp)
    } else if name_lc.ends_with(".lz4") {
        extract_single(
            lz4_flex::frame::FrameDecoder::new(open_buf(archive_path)?),
            tmp,
        )
    } else if name_lc.ends_with(".rar") {
        extract_rar(archive_path, inner_path, password, tmp)
    } else {
        Err(ArchiveError::Other(format!(
            "Unsupported format: {}",
            archive_path.display()
        )))
    }
}

// ─── RAR ─────────────────────────────────────────────────────────────────────
//
// No pure-Rust RAR crate exists (RAR is proprietary). We shell out to `unar`
// (The Unarchiver) or `unrar` as a fallback.
// Install: sudo apt install unar   OR   sudo pacman -S unar

/// Returns the first RAR CLI tool found on PATH, or `None` if neither is available.
fn rar_tool() -> Option<&'static str> {
    ["unar", "unrar"]
        .iter()
        .find(|&&tool| {
            std::process::Command::new(tool)
                .arg("--version")
                .output()
                .is_ok()
        })
        .copied()
}

fn list_rar(
    archive_path: &Path,
    prefix: &str,
    password: Option<&str>,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let tool = rar_tool().ok_or_else(|| {
        ArchiveError::Other(
            "RAR support requires 'unar' or 'unrar' (install via your package manager)".to_owned(),
        )
    })?;

    let mut cmd = std::process::Command::new(tool);
    match tool {
        "unar" => {
            cmd.arg("-list").arg(archive_path);
            if let Some(pwd) = password {
                cmd.arg("-password").arg(pwd);
            }
        }
        _ => {
            cmd.arg("l");
            cmd.arg(if let Some(pwd) = password {
                format!("-p{pwd}")
            } else {
                "-p-".to_owned()
            });
            cmd.arg(archive_path);
        }
    }

    let output = cmd
        .output()
        .map_err(|e| ArchiveError::Other(format!("{tool} spawn: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("password") || stderr.contains("encrypted") {
            return Err(if password.is_none() {
                ArchiveError::PasswordRequired
            } else {
                ArchiveError::WrongPassword
            });
        }
        return Err(ArchiveError::Other(format!(
            "{tool} failed ({}): {}",
            output.status,
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut seen: HashMap<String, ArchiveEntry> = HashMap::new();

    if tool == "unar" {
        parse_unar_list(&stdout, prefix, &mut seen);
    } else {
        parse_unrar_list(&stdout, prefix, &mut seen);
    }

    Ok(seen.into_values().collect())
}

/// Parses `unar -list` tab-separated output, name is the last tab-delimited field.
fn parse_unar_list(stdout: &str, prefix: &str, seen: &mut HashMap<String, ArchiveEntry>) {
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("Archive")
            || line.starts_with("---")
            || line.starts_with("===")
        {
            continue;
        }
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        let raw_name = parts
            .last()
            .map(|s| s.trim().replace('\\', "/"))
            .unwrap_or_default();
        let raw_name = raw_name.trim_end_matches('/');
        if raw_name.is_empty() {
            continue;
        }
        let size: u64 = parts
            .first()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let is_dir = size == 0 && raw_name.contains('/');
        collect_entry(seen, raw_name, is_dir, size, 0, prefix);
    }
}

/// Parses `unrar l` fixed-width output, listing is delimited by `---` separator lines.
fn parse_unrar_list(stdout: &str, prefix: &str, seen: &mut HashMap<String, ArchiveEntry>) {
    let mut in_listing = false;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("---") {
            in_listing = !in_listing;
            continue;
        }
        if !in_listing || trimmed.is_empty() {
            continue;
        }
        let raw_name = match line.split_whitespace().last() {
            Some(n) => n.replace('\\', "/"),
            None => continue,
        };
        let raw_name = raw_name.trim_end_matches('/').to_owned();
        if raw_name.is_empty() {
            continue;
        }
        let is_dir = line
            .chars()
            .next()
            .map(|c| c == 'd' || c == 'D')
            .unwrap_or(false);
        let size: u64 = line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        collect_entry(seen, &raw_name, is_dir, size, 0, prefix);
    }
}

fn extract_rar(
    archive_path: &Path,
    inner_path: &str,
    password: Option<&str>,
    mut tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    let tool = rar_tool().ok_or_else(|| {
        ArchiveError::Other(
            "RAR support requires 'unar' or 'unrar' (install via your package manager)".to_owned(),
        )
    })?;

    let tmp_dir = tempfile::tempdir().map_err(|e| ArchiveError::Other(format!("tempdir: {e}")))?;

    let mut cmd = std::process::Command::new(tool);
    match tool {
        "unar" => {
            cmd.arg("-output-directory")
                .arg(tmp_dir.path())
                .arg(archive_path)
                .arg(inner_path);
            if let Some(pwd) = password {
                cmd.arg("-password").arg(pwd);
            }
        }
        _ => {
            cmd.arg("e");
            cmd.arg(if let Some(pwd) = password {
                format!("-p{pwd}")
            } else {
                "-p-".to_owned()
            });
            cmd.arg(archive_path).arg(inner_path).arg(tmp_dir.path());
        }
    }

    let output = cmd
        .output()
        .map_err(|e| ArchiveError::Other(format!("{tool} spawn: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("password") || stderr.contains("encrypted") {
            return Err(if password.is_none() {
                ArchiveError::PasswordRequired
            } else {
                ArchiveError::WrongPassword
            });
        }
        return Err(ArchiveError::Other(format!(
            "{tool} extract failed: {}",
            stderr.trim()
        )));
    }

    let file_name = std::path::Path::new(inner_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(inner_path);

    let extracted = find_file_recursive(tmp_dir.path(), file_name)?;

    let mut src = std::fs::File::open(&extracted)
        .map_err(|e| ArchiveError::Other(format!("open extracted: {e}")))?;
    std::io::copy(&mut src, &mut tmp).map_err(|e| ArchiveError::Other(format!("copy: {e}")))?;
    tmp.flush()
        .map_err(|e| ArchiveError::Other(format!("flush: {e}")))?;
    Ok(tmp)
}

/// Walks `dir` recursively to find the first file whose name matches `file_name`.
fn find_file_recursive(dir: &Path, file_name: &str) -> Result<PathBuf, ArchiveError> {
    for entry in std::fs::read_dir(dir).map_err(|e| ArchiveError::Other(format!("readdir: {e}")))? {
        let entry = entry.map_err(|e| ArchiveError::Other(format!("entry: {e}")))?;
        let path = entry.path();
        if path.is_dir() {
            if let Ok(found) = find_file_recursive(&path, file_name) {
                return Ok(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(file_name) {
            return Ok(path);
        }
    }
    Err(ArchiveError::Other(format!(
        "extracted file not found: {file_name}"
    )))
}

// ─── ZIP ─────────────────────────────────────────────────────────────────────

fn list_zip(
    archive_path: &Path,
    prefix: &str,
    password: Option<&str>,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| ArchiveError::Other(format!("open: {e}")))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| ArchiveError::Other(format!("ZIP: {e}")))?;

    // 1. If no password was provided, check raw headers for encryption flags
    if password.is_none() {
        for i in 0..zip.len() {
            let is_enc = match zip.by_index_raw(i) {
                Ok(entry) => entry.is_file() && entry.encrypted(),
                Err(_) => false,
            };
            if is_enc {
                return Err(ArchiveError::PasswordRequired);
            }
        }
    }

    // 2. We either have a password or the archive is unencrypted: process entries
    let mut seen: HashMap<String, ArchiveEntry> = HashMap::new();

    for i in 0..zip.len() {
        let (raw_name, is_dir, size, mtime) = match password {
            Some(pwd) => match zip.by_index_decrypt(i, pwd.as_bytes()) {
                Ok(entry) => {
                    let n = entry.name().replace('\\', "/");
                    let n = n.trim_end_matches('/').to_owned();
                    let d = entry.is_dir();
                    let s = entry.size();
                    let t = zip_mtime(&entry);
                    (n, d, s, t)
                }
                Err(zip::result::ZipError::UnsupportedArchive(_))
                | Err(zip::result::ZipError::InvalidPassword) => {
                    return Err(ArchiveError::WrongPassword);
                }
                Err(e) => return Err(ArchiveError::Other(format!("ZIP idx {i}: {e}"))),
            },
            None => match zip.by_index_raw(i) {
                Ok(entry) => {
                    let n = entry.name().replace('\\', "/");
                    let n = n.trim_end_matches('/').to_owned();
                    let d = entry.is_dir();
                    let s = entry.size();
                    let t = zip_mtime(&entry);
                    (n, d, s, t)
                }
                Err(e) => return Err(ArchiveError::Other(format!("ZIP idx {i}: {e}"))),
            },
        };

        collect_entry(&mut seen, &raw_name, is_dir, size, mtime, prefix);
    }

    Ok(seen.into_values().collect())
}

/// Extracts a folder entry and all of its nested contents from an archive into a
/// temporary directory under `/tmp`.
///
/// Returns the `PathBuf` pointing to the extracted root folder.
#[allow(dead_code)]
pub fn extract_dir_to_tempdir(
    archive_path: &std::path::Path,
    inner_dir: &str,
    _password: Option<&str>,
) -> Result<std::path::PathBuf, ArchiveError> {
    let folder_name = std::path::Path::new(inner_dir)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "folder".to_string());

    let temp_dir = tempfile::Builder::new()
        .prefix(&format!(".tmp.{}.", folder_name))
        .tempdir_in(std::env::temp_dir())
        .map_err(|e| ArchiveError::Other(format!("tempdir creation failed: {e}")))?;

    let dest_dir = temp_dir.path().join(&folder_name);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| ArchiveError::Other(format!("create_dir_all failed: {e}")))?;

    let prefix = if inner_dir.ends_with('/') {
        inner_dir.to_string()
    } else {
        format!("{}/", inner_dir)
    };

    let file = std::fs::File::open(archive_path)
        .map_err(|e| ArchiveError::Other(format!("open archive failed: {e}")))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| ArchiveError::Other(format!("ZIP parse failed: {e}")))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ArchiveError::Other(format!("read entry failed: {e}")))?;
        let name = entry.name().to_string();

        if name.starts_with(&prefix) {
            let relative_path = name.trim_start_matches(&prefix);
            if relative_path.is_empty() {
                continue;
            }

            let out_path = dest_dir.join(relative_path);
            if entry.is_dir() {
                std::fs::create_dir_all(&out_path)
                    .map_err(|e| ArchiveError::Other(format!("create dir failed: {e}")))?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        ArchiveError::Other(format!("create parent dir failed: {e}"))
                    })?;
                }
                let mut outfile = std::fs::File::create(&out_path)
                    .map_err(|e| ArchiveError::Other(format!("create file failed: {e}")))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| ArchiveError::Other(format!("copy entry failed: {e}")))?;
            }
        }
    }

    let result_path = dest_dir.clone();
    std::mem::forget(temp_dir);
    Ok(result_path)
}

fn extract_zip(
    archive_path: &Path,
    inner_path: &str,
    password: Option<&str>,
    mut tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| ArchiveError::Other(format!("open: {e}")))?;

    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| ArchiveError::Other(format!("ZIP: {e}")))?;

    let idx = zip
        .index_for_name(inner_path)
        .ok_or_else(|| ArchiveError::Other(format!("not found: {inner_path}")))?;

    if let Some(pwd) = password {
        // Password supplied: try decrypting
        match zip.by_index_decrypt(idx, pwd.as_bytes()) {
            Ok(mut entry) => {
                std::io::copy(&mut entry, &mut tmp)
                    .map_err(|e| ArchiveError::Other(format!("copy: {e}")))?;
                tmp.flush()
                    .map_err(|e| ArchiveError::Other(format!("flush: {e}")))?;
                Ok(tmp)
            }
            Err(zip::result::ZipError::UnsupportedArchive(_))
            | Err(zip::result::ZipError::InvalidPassword) => Err(ArchiveError::WrongPassword),
            Err(e) => Err(ArchiveError::Other(format!("ZIP decrypt read: {e}"))),
        }
    } else {
        // No password supplied: try standard read and catch password requirements
        match zip.by_index(idx) {
            Ok(mut entry) => {
                if entry.encrypted() {
                    return Err(ArchiveError::PasswordRequired);
                }
                std::io::copy(&mut entry, &mut tmp)
                    .map_err(|e| ArchiveError::Other(format!("copy: {e}")))?;
                tmp.flush()
                    .map_err(|e| ArchiveError::Other(format!("flush: {e}")))?;
                Ok(tmp)
            }
            Err(zip::result::ZipError::UnsupportedArchive(_))
            | Err(zip::result::ZipError::InvalidPassword) => Err(ArchiveError::PasswordRequired),
            Err(e) => Err(ArchiveError::Other(format!("ZIP read: {e}"))),
        }
    }
}

// ─── 7-Zip ───────────────────────────────────────────────────────────────────

fn list_7z(
    archive_path: &Path,
    prefix: &str,
    password: Option<&str>,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| ArchiveError::Other(format!("open: {e}")))?;

    let pwd: Password = password.map(Password::from).unwrap_or_else(Password::empty);

    let reader = ArchiveReader::new(file, pwd).map_err(|e| match e {
        sevenz_rust2::Error::PasswordRequired => ArchiveError::PasswordRequired,
        sevenz_rust2::Error::MaybeBadPassword(_) => {
            if password.is_none() {
                ArchiveError::PasswordRequired
            } else {
                ArchiveError::WrongPassword
            }
        }
        _ => {
            let msg = e.to_string();
            let is_pwd_err = msg.contains("Password")
                || msg.contains("password")
                || msg.contains("Decrypt")
                || msg.contains("unexpectedEof")
                || msg.contains("UnexpectedEof")
                || msg.contains("failed to fill whole buffer");

            if is_pwd_err {
                if password.is_none() {
                    ArchiveError::PasswordRequired
                } else {
                    ArchiveError::WrongPassword
                }
            } else {
                ArchiveError::Other(msg)
            }
        }
    })?;

    let mut seen: HashMap<String, ArchiveEntry> = HashMap::new();

    for entry in &reader.archive().files.clone() {
        let raw_name = entry.name().replace('\\', "/");
        let raw_name_trimmed = raw_name.trim_end_matches('/');
        if raw_name_trimmed.is_empty() {
            continue;
        }
        let is_dir = entry.is_directory;
        let size = entry.size;
        let mtime = std::time::SystemTime::from(entry.last_modified_date())
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        collect_entry(&mut seen, raw_name_trimmed, is_dir, size, mtime, prefix);
    }

    Ok(seen.into_values().collect())
}

fn extract_7z(
    archive_path: &Path,
    inner_path: &str,
    password: Option<&str>,
    mut tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| ArchiveError::Other(format!("open: {e}")))?;

    let pwd: Password = password.map(Password::from).unwrap_or_else(Password::empty);

    let mut reader = ArchiveReader::new(file, pwd.clone()).map_err(|e| match e {
        sevenz_rust2::Error::PasswordRequired => ArchiveError::PasswordRequired,
        sevenz_rust2::Error::MaybeBadPassword(_) => {
            if password.is_none() {
                ArchiveError::PasswordRequired
            } else {
                ArchiveError::WrongPassword
            }
        }
        _ => {
            let msg = e.to_string();
            let is_pwd_err = msg.contains("Password")
                || msg.contains("password")
                || msg.contains("Decrypt")
                || msg.contains("unexpectedEof")
                || msg.contains("UnexpectedEof")
                || msg.contains("failed to fill whole buffer");

            if is_pwd_err {
                if password.is_none() {
                    ArchiveError::PasswordRequired
                } else {
                    ArchiveError::WrongPassword
                }
            } else {
                ArchiveError::Other(msg)
            }
        }
    })?;

    let mut found = false;
    reader
        .for_each_entries(
            &mut |entry: &sevenz_rust2::ArchiveEntry, r: &mut dyn std::io::Read| {
                let n = entry.name().replace('\\', "/");
                if n.trim_end_matches('/') == inner_path {
                    std::io::copy(r, &mut tmp).map_err(sevenz_rust2::Error::from)?;
                    found = true;
                    Ok(false) // stop after first match
                } else {
                    std::io::copy(r, &mut std::io::sink()).map_err(sevenz_rust2::Error::from)?;
                    Ok(true)
                }
            },
        )
        .map_err(|e| match e {
            sevenz_rust2::Error::PasswordRequired => ArchiveError::PasswordRequired,
            sevenz_rust2::Error::MaybeBadPassword(_) => {
                if password.is_none() {
                    ArchiveError::PasswordRequired
                } else {
                    ArchiveError::WrongPassword
                }
            }
            _ => {
                let msg = e.to_string();
                let is_pwd_err = msg.contains("Password")
                    || msg.contains("password")
                    || msg.contains("Decrypt")
                    || msg.contains("unexpectedEof")
                    || msg.contains("UnexpectedEof")
                    || msg.contains("failed to fill whole buffer");

                if is_pwd_err {
                    if password.is_none() {
                        ArchiveError::PasswordRequired
                    } else {
                        ArchiveError::WrongPassword
                    }
                } else {
                    ArchiveError::Other(msg)
                }
            }
        })?;

    if !found {
        return Err(ArchiveError::Other(format!("not found: {inner_path}")));
    }

    tmp.flush()
        .map_err(|e| ArchiveError::Other(format!("flush: {e}")))?;
    Ok(tmp)
}
// ─── TAR (all compression flavours) ──────────────────────────────────────────

fn list_tar<R: Read>(reader: R, prefix: &str) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let mut archive = tar::Archive::new(reader);
    let mut seen: HashMap<String, ArchiveEntry> = HashMap::new();

    for entry in archive
        .entries()
        .map_err(|e| ArchiveError::Other(format!("TAR: {e}")))?
    {
        let entry = entry.map_err(|e| ArchiveError::Other(format!("TAR entry: {e}")))?;
        let header = entry.header();
        let raw_name = entry
            .path()
            .map_err(|e| ArchiveError::Other(format!("TAR path: {e}")))?
            .to_string_lossy()
            .replace('\\', "/");
        let raw_name = raw_name.trim_end_matches('/');
        let is_dir = header.entry_type().is_dir();
        let size = header.size().unwrap_or(0);
        let mtime = header.mtime().unwrap_or(0) as i64;
        collect_entry(&mut seen, raw_name, is_dir, size, mtime, prefix);
    }

    Ok(seen.into_values().collect())
}

fn extract_tar<R: Read>(
    reader: R,
    inner_path: &str,
    mut tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    let mut archive = tar::Archive::new(reader);
    for entry in archive
        .entries()
        .map_err(|e| ArchiveError::Other(format!("TAR: {e}")))?
    {
        let mut entry = entry.map_err(|e| ArchiveError::Other(format!("TAR entry: {e}")))?;
        let path = entry
            .path()
            .map_err(|e| ArchiveError::Other(format!("TAR path: {e}")))?
            .to_string_lossy()
            .replace('\\', "/");
        if path.trim_end_matches('/') == inner_path {
            std::io::copy(&mut entry, &mut tmp)
                .map_err(|e| ArchiveError::Other(format!("copy: {e}")))?;
            tmp.flush()
                .map_err(|e| ArchiveError::Other(format!("flush: {e}")))?;
            return Ok(tmp);
        }
    }
    Err(ArchiveError::Other(format!("not found: {inner_path}")))
}

// ─── Standalone compressed single-file archives ───────────────────────────────

/// Produces a listing with a single virtual entry - the decompressed filename.
/// Used for `.gz`, `.bz2`, `.xz`, `.zst`, `.lz4` files that are not tarballs.
fn single_file_listing(
    archive_path: &Path,
    inner_name: &str,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    if inner_name.is_empty() {
        return Ok(vec![]);
    }
    Ok(vec![ArchiveEntry {
        name: inner_name.to_owned(),
        is_dir: false,
        // Size is unknown without full decompression, 0 is acceptable for display.
        size: 0,
        mtime: archive_path
            .metadata()
            .ok()
            .and_then(|m| {
                m.modified().ok().and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_secs() as i64)
                })
            })
            .unwrap_or(0),
        inner_path: inner_name.to_owned(),
    }])
}

fn extract_single<R: Read>(
    mut reader: R,
    mut tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    std::io::copy(&mut reader, &mut tmp)
        .map_err(|e| ArchiveError::Other(format!("decompress: {e}")))?;
    tmp.flush()
        .map_err(|e| ArchiveError::Other(format!("flush: {e}")))?;
    Ok(tmp)
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

/// Resolves one archive entry path against `prefix` and inserts the immediate
/// child (real or synthesised directory node) into `seen`.
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
            _ => return,
        }
    };

    let (child_name, is_dir, child_inner) = match relative.find('/') {
        Some(slash_pos) => {
            let dir_name = &relative[..slash_pos];
            let inner = if prefix.is_empty() {
                dir_name.to_owned()
            } else {
                format!("{}/{}", prefix.trim_end_matches('/'), dir_name)
            };
            (dir_name.to_owned(), true, inner)
        }
        None => {
            let inner = if prefix.is_empty() {
                relative.clone()
            } else {
                format!("{}/{}", prefix.trim_end_matches('/'), relative)
            };
            // Only set is_dir if explicitly flagged as directory AND not a leaf filename
            (relative, is_entry_dir, inner)
        }
    };

    // Synthesized directory nodes must always have size = 0
    let entry_size = if is_dir { 0 } else { size };

    seen.entry(child_name.clone())
        .and_modify(|existing| {
            // If we encounter a real file payload for a synthesised directory node,
            // preserve the real entry metadata.
            if !is_dir {
                existing.is_dir = false;
                existing.size = size;
                existing.mtime = mtime;
            }
        })
        .or_insert(ArchiveEntry {
            name: child_name,
            is_dir,
            size: entry_size,
            mtime,
            inner_path: child_inner,
        });
}

#[inline]
fn open_buf(path: &Path) -> Result<BufReader<std::fs::File>, ArchiveError> {
    std::fs::File::open(path)
        .map(BufReader::new)
        .map_err(|e| ArchiveError::Other(format!("open: {e}")))
}

#[inline]
fn lc_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

#[inline]
fn strip_ext<'a>(name: &'a str, ext: &str) -> &'a str {
    name.strip_suffix(ext).unwrap_or(name)
}

/// Extracts a last-modified unix timestamp from a ZIP entry's `last_modified` field.
fn zip_mtime<R: Read + Seek>(entry: &zip::read::ZipFile<'_, R>) -> i64 {
    entry
        .last_modified()
        .map(|dt| {
            let y = dt.year() as i64;
            let m = dt.month() as i64;
            let d = dt.day() as i64;
            let h = dt.hour() as i64;
            let min = dt.minute() as i64;
            let s = dt.second() as i64;
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
            ("archive.7z", true),
            ("archive.rar", true),
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
        let entries = list_archive_entries(&zip_path, "", None).unwrap();
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
        let entries = list_archive_entries(&zip_path, "dir", None).unwrap();
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
        let entries = list_archive_entries(&zip_path, "dir/sub", None).unwrap();
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
        let entries = list_archive_entries(&tar_path, "", None).unwrap();
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
        let entries = list_archive_entries(&path, "", None).unwrap();
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
        let entries = list_archive_entries(&path, "", None).unwrap();
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
        let entries = list_archive_entries(&path, "", None).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_list_archive_empty_zip() {
        let dir = tempdir().unwrap();
        let zip_path = dir.path().join("empty.zip");
        File::create(&zip_path).unwrap(); // empty file, not a valid zip
        let result = list_archive_entries(&zip_path, "", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_archive_nonexistent() {
        let path = Path::new("/nonexistent/file.zip");
        let result = list_archive_entries(path, "", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_archive_unsupported_format() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.unsupported");
        fs::write(&path, b"dummy").unwrap();
        let result = list_archive_entries(&path, "", None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unsupported format"));
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
        let tmp = extract_entry_to_tempfile(&zip_path, "file1.txt", None).unwrap();
        let content = fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "Hello");
        // Keep the temp file alive until end of test.
        tmp.keep().ok();
    }

    #[test]
    fn test_extract_zip_entry_in_subdir() {
        let (_dir, zip_path) = create_test_zip();
        let tmp = extract_entry_to_tempfile(&zip_path, "dir/file2.txt", None).unwrap();
        let content = fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "World");
        tmp.keep().ok();
    }

    #[test]
    fn test_extract_tar_entry() {
        let (_dir, tar_path) = create_test_tar();
        let tmp = extract_entry_to_tempfile(&tar_path, "file1.txt", None).unwrap();
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
        let tmp = extract_entry_to_tempfile(&path, "file1.txt", None).unwrap();
        let content = fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "Hello");
        tmp.keep().ok();
    }

    #[test]
    fn test_extract_entry_not_found() {
        let (_dir, zip_path) = create_test_zip();
        let result = extract_entry_to_tempfile(&zip_path, "nonexistent.txt", None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_extract_from_unsupported_archive() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.unsupported");
        fs::write(&path, b"dummy").unwrap();
        let result = extract_entry_to_tempfile(&path, "file", None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unsupported format"));
    }
}
