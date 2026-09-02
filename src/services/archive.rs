//! Virtual read-only filesystem layer for browsing compressed archives.
//!
//! Implements a URI scheme `/archive://<encoded_host>/<inner_path>` where the
//! host component is the percent-encoded absolute path of the archive on disk.
//!
//! # Supported formats
//! | Extension                                  | Backend          | Password |
//! |--------------------------------------------|------------------|----------|
//! | `.zip`                                     | `zip`            | ✓        |
//! | `.tar`, `.tar.gz`, `.tgz`                  | `tar` + `flate2` | ✗        |
//! | `.tar.bz2`, `.tbz2`                        | `tar` + `bzip2`  | ✗        |
//! | `.tar.xz`, `.txz`                          | `tar` + `xz2`    | ✗        |
//! | `.tar.lzma`, `.tlz`                        | `tar` + `xz2`    | ✗        |
//! | `.tar.zst`, `.tzst`                        | `tar` + `zstd`   | ✗        |
//! | `.tar.lz4`                                 | `tar` + `lz4`    | ✗        |
//! | `.7z`                                      | `sevenz-rust2`   | ✓        |
//! | `.iso` (ISO 9660)                          | `iso9660_simple` | ✗        |
//! | `.iso` (UDF)                               | `7z` (p7zip)     | ✗        |
//! | `.gz` (standalone)                         | `flate2`         | ✗        |
//! | `.bz2` (standalone)                        | `bzip2`          | ✗        |
//! | `.xz` / `.lzma` (standalone)               | `xz2`            | ✗        |
//! | `.zst` / `.zstd` (standalone)              | `zstd`           | ✗        |
//! | `.lz4` (standalone)                        | `lz4_flex`       | ✗        |
//! | `.deb` (Debian package)                    | `ar` + `tar`     | ✗        |

use std::collections::HashMap;
use std::io::{BufReader, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;

use crate::model::FileLoadContext;
use sevenz_rust2::{ArchiveReader, Password};

static FLUX_TEMP_DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
static ACTIVE_TEMP_DIRS: Mutex<Vec<tempfile::TempDir>> = Mutex::new(Vec::new());

/// Registers an extracted directory guard so it stays alive while needed,
/// but can be cleared when browsing elsewhere.
pub fn register_temp_dir(dir: tempfile::TempDir) {
    if let Ok(mut lock) = ACTIVE_TEMP_DIRS.lock() {
        lock.push(dir);
    }
}

/// Cleans up all extracted archive temp directories from `/tmp`.
pub fn clear_archive_temp_dirs() {
    if let Ok(mut lock) = ACTIVE_TEMP_DIRS.lock() {
        lock.clear();
    }
}

/// Returns a shared scratch folder for Flux that is automatically purged by RAII on exit.
pub fn flux_scratch_dir() -> &'static Path {
    FLUX_TEMP_DIR
        .get_or_init(|| {
            tempfile::Builder::new()
                .prefix(&format!("flux-{}-", std::process::id()))
                .tempdir()
                .expect("failed to create flux temp scratch dir")
        })
        .path()
}

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
            "zip", "tar", "tar.gz", "tgz", "tar.bz2", "tbz2", "tar.xz", "txz", "tar.lzma", "tlz",
            "tar.zst", "tzst", "tar.lz4", "7z", "rar", "gz", "bz2", "xz", "lzma", "zst", "zstd",
            "lz4", "iso", "deb",
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
#[derive(Debug, Clone)]
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
            ArchiveError::PasswordRequired => {
                write!(f, "{}", crate::i18n::tr("Archive is password-protected"))
            }
            ArchiveError::WrongPassword => write!(f, "{}", crate::i18n::tr("Incorrect password")),
            ArchiveError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<String> for ArchiveError {
    fn from(s: String) -> Self {
        ArchiveError::Other(s)
    }
}

// ─── ArchiveBackend trait ────────────────────────────────────────────────────

/// Common interface for all archive format backends.
pub trait ArchiveBackend: Send + Sync {
    /// List entries at `prefix` inside the archive.
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError>;

    /// Extract a single entry as raw bytes.
    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        inner_path: &str,
        password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError>;

    /// Extract a directory subtree to a temporary directory and return its root path.
    fn extract_dir(
        &self,
        archive_path: &Path,
        inner_dir: &str,
        password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError>;
}

// ─── Real backends ────────────────────────────────────────────────────────────

struct ZipBackend;
struct TarBackend;
struct SevenZBackend;
struct RarBackend;
struct IsoBackend;
struct SingleFileBackend; // for .gz, .bz2, .xz, .lzma, .zst, .lz4
struct DebBackend; // for .deb (ar + inner data.tar.*)
struct UnsupportedBackend;

impl ArchiveBackend for ZipBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        list_zip(archive_path, prefix, password)
    }

    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        inner_path: &str,
        password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        extract_bytes_via_tempfile(archive_path, inner_path, password, |p, i, pw, tmp| {
            extract_zip(p, i, pw, tmp)
        })
    }

    fn extract_dir(
        &self,
        archive_path: &Path,
        inner_dir: &str,
        password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        extract_dir_zip(archive_path, inner_dir, password)
    }
}

impl ArchiveBackend for TarBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        _password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        let name_lc = lc_name(archive_path);
        if name_lc.ends_with(".tar.gz") || name_lc.ends_with(".tgz") {
            list_tar(
                flate2::read::GzDecoder::new(open_buf(archive_path)?),
                prefix,
            )
        } else if name_lc.ends_with(".tar.bz2") || name_lc.ends_with(".tbz2") {
            list_tar(bzip2::read::BzDecoder::new(open_buf(archive_path)?), prefix)
        } else if name_lc.ends_with(".tar.xz")
            || name_lc.ends_with(".txz")
            || name_lc.ends_with(".tar.lzma")
            || name_lc.ends_with(".tlz")
        {
            list_tar(xz2::read::XzDecoder::new(open_buf(archive_path)?), prefix)
        } else if name_lc.ends_with(".tar.zst") || name_lc.ends_with(".tzst") {
            let dec = zstd::stream::read::Decoder::new(open_buf(archive_path)?)
                .map_err(|e| ArchiveError::Other(format!("zstd open: {e}")))?;
            list_tar(dec, prefix)
        } else if name_lc.ends_with(".tar.lz4") {
            let dec = lz4_flex::frame::FrameDecoder::new(open_buf(archive_path)?);
            list_tar(dec, prefix)
        } else {
            // plain .tar
            list_tar(open_buf(archive_path)?, prefix)
        }
    }

    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        inner_path: &str,
        password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        extract_bytes_via_tempfile(archive_path, inner_path, password, |p, i, _pw, tmp| {
            let name_lc = lc_name(p);
            if name_lc.ends_with(".tar.gz") || name_lc.ends_with(".tgz") {
                extract_tar(flate2::read::GzDecoder::new(open_buf(p)?), i, tmp)
            } else if name_lc.ends_with(".tar.bz2") || name_lc.ends_with(".tbz2") {
                extract_tar(bzip2::read::BzDecoder::new(open_buf(p)?), i, tmp)
            } else if name_lc.ends_with(".tar.xz")
                || name_lc.ends_with(".txz")
                || name_lc.ends_with(".tar.lzma")
                || name_lc.ends_with(".tlz")
            {
                extract_tar(xz2::read::XzDecoder::new(open_buf(p)?), i, tmp)
            } else if name_lc.ends_with(".tar.zst") || name_lc.ends_with(".tzst") {
                let dec = zstd::stream::read::Decoder::new(open_buf(p)?)
                    .map_err(|e| ArchiveError::Other(format!("zstd: {e}")))?;
                extract_tar(dec, i, tmp)
            } else if name_lc.ends_with(".tar.lz4") {
                extract_tar(lz4_flex::frame::FrameDecoder::new(open_buf(p)?), i, tmp)
            } else {
                extract_tar(open_buf(p)?, i, tmp)
            }
        })
    }

    fn extract_dir(
        &self,
        archive_path: &Path,
        inner_dir: &str,
        _password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        let name_lc = lc_name(archive_path);
        if name_lc.ends_with(".tar.gz") || name_lc.ends_with(".tgz") {
            extract_dir_tar(
                flate2::read::GzDecoder::new(open_buf(archive_path)?),
                inner_dir,
            )
        } else if name_lc.ends_with(".tar.bz2") || name_lc.ends_with(".tbz2") {
            extract_dir_tar(
                bzip2::read::BzDecoder::new(open_buf(archive_path)?),
                inner_dir,
            )
        } else if name_lc.ends_with(".tar.xz")
            || name_lc.ends_with(".txz")
            || name_lc.ends_with(".tar.lzma")
            || name_lc.ends_with(".tlz")
        {
            extract_dir_tar(
                xz2::read::XzDecoder::new(open_buf(archive_path)?),
                inner_dir,
            )
        } else if name_lc.ends_with(".tar.zst") || name_lc.ends_with(".tzst") {
            let dec = zstd::stream::read::Decoder::new(open_buf(archive_path)?)
                .map_err(|e| ArchiveError::Other(format!("zstd: {e}")))?;
            extract_dir_tar(dec, inner_dir)
        } else if name_lc.ends_with(".tar.lz4") {
            extract_dir_tar(
                lz4_flex::frame::FrameDecoder::new(open_buf(archive_path)?),
                inner_dir,
            )
        } else {
            extract_dir_tar(open_buf(archive_path)?, inner_dir)
        }
    }
}

impl ArchiveBackend for SevenZBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        list_7z(archive_path, prefix, password)
    }

    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        inner_path: &str,
        password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        extract_bytes_via_tempfile(archive_path, inner_path, password, |p, i, pw, tmp| {
            extract_7z(p, i, pw, tmp)
        })
    }

    fn extract_dir(
        &self,
        archive_path: &Path,
        inner_dir: &str,
        password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        extract_dir_7z(archive_path, inner_dir, password)
    }
}

impl ArchiveBackend for RarBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        list_rar(archive_path, prefix, password)
    }

    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        inner_path: &str,
        password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        extract_bytes_via_tempfile(archive_path, inner_path, password, |p, i, pw, tmp| {
            extract_rar(p, i, pw, tmp)
        })
    }

    fn extract_dir(
        &self,
        _archive_path: &Path,
        _inner_dir: &str,
        _password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        Err(ArchiveError::Other(
            "Directory extraction from RAR archives is not supported yet".to_string(),
        ))
    }
}

impl ArchiveBackend for IsoBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        _password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        list_iso(archive_path, prefix)
    }

    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        inner_path: &str,
        password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        extract_bytes_via_tempfile(archive_path, inner_path, password, |p, i, _pw, tmp| {
            extract_iso(p, i, tmp)
        })
    }

    fn extract_dir(
        &self,
        _archive_path: &Path,
        _inner_dir: &str,
        _password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        Err(ArchiveError::Other(
            "Directory extraction from ISO images is not supported yet".to_string(),
        ))
    }
}

impl ArchiveBackend for SingleFileBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        _prefix: &str,
        _password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        let name_lc = lc_name(archive_path);
        let inner_name = if name_lc.ends_with(".gz") {
            strip_ext(&name_lc, ".gz")
        } else if name_lc.ends_with(".bz2") {
            strip_ext(&name_lc, ".bz2")
        } else if name_lc.ends_with(".xz") {
            strip_ext(&name_lc, ".xz")
        } else if name_lc.ends_with(".lzma") {
            strip_ext(&name_lc, ".lzma")
        } else if name_lc.ends_with(".zstd") {
            strip_ext(&name_lc, ".zstd")
        } else if name_lc.ends_with(".zst") {
            strip_ext(&name_lc, ".zst")
        } else if name_lc.ends_with(".lz4") {
            strip_ext(&name_lc, ".lz4")
        } else {
            return Err(ArchiveError::Other("Unsupported single-file format".into()));
        };
        single_file_listing(archive_path, inner_name)
    }

    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        _inner_path: &str,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        // Extract the whole file (the only entry) to bytes.
        let name_lc = lc_name(archive_path);
        let reader: Box<dyn Read> = if name_lc.ends_with(".gz") {
            Box::new(flate2::read::GzDecoder::new(open_buf(archive_path)?))
        } else if name_lc.ends_with(".bz2") {
            Box::new(bzip2::read::BzDecoder::new(open_buf(archive_path)?))
        } else if name_lc.ends_with(".xz") || name_lc.ends_with(".lzma") {
            Box::new(xz2::read::XzDecoder::new(open_buf(archive_path)?))
        } else if name_lc.ends_with(".zstd") || name_lc.ends_with(".zst") {
            let dec = zstd::stream::read::Decoder::new(open_buf(archive_path)?)
                .map_err(|e| ArchiveError::Other(format!("zstd: {e}")))?;
            Box::new(dec)
        } else if name_lc.ends_with(".lz4") {
            Box::new(lz4_flex::frame::FrameDecoder::new(open_buf(archive_path)?))
        } else {
            return Err(ArchiveError::Other("Unsupported single-file format".into()));
        };
        let mut data = Vec::new();
        let mut reader = reader;
        std::io::copy(&mut reader, &mut data)
            .map_err(|e| ArchiveError::Other(format!("decompress: {e}")))?;
        Ok(data)
    }

    fn extract_dir(
        &self,
        _archive_path: &Path,
        _inner_dir: &str,
        _password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        Err(ArchiveError::Other(
            "Directory extraction from single-file archives is not supported".into(),
        ))
    }
}

impl ArchiveBackend for DebBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        _password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        list_deb(archive_path, prefix)
    }

    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        inner_path: &str,
        password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        extract_bytes_via_tempfile(archive_path, inner_path, password, |p, i, _pw, tmp| {
            extract_deb(p, i, tmp)
        })
    }

    fn extract_dir(
        &self,
        archive_path: &Path,
        inner_dir: &str,
        _password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        extract_dir_deb(archive_path, inner_dir)
    }
}

impl ArchiveBackend for UnsupportedBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        _prefix: &str,
        _password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        Err(ArchiveError::Other(format!(
            "Unsupported format: {}",
            archive_path.display()
        )))
    }

    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        _inner_path: &str,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        Err(ArchiveError::Other(format!(
            "Unsupported format: {}",
            archive_path.display()
        )))
    }

    fn extract_dir(
        &self,
        archive_path: &Path,
        _inner_dir: &str,
        _password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        Err(ArchiveError::Other(format!(
            "Unsupported format: {}",
            archive_path.display()
        )))
    }
}

// ─── Factory ─────────────────────────────────────────────────────────────────

fn get_backend(
    archive_path: &Path,
    backend: Option<Box<dyn ArchiveBackend>>,
) -> Box<dyn ArchiveBackend> {
    if let Some(b) = backend {
        return b;
    }
    let name_lc = lc_name(archive_path);
    if name_lc.ends_with(".zip") {
        Box::new(ZipBackend)
    } else if name_lc.ends_with(".7z") {
        Box::new(SevenZBackend)
    } else if name_lc.ends_with(".tar.gz")
        || name_lc.ends_with(".tgz")
        || name_lc.ends_with(".tar.bz2")
        || name_lc.ends_with(".tbz2")
        || name_lc.ends_with(".tar.xz")
        || name_lc.ends_with(".txz")
        || name_lc.ends_with(".tar.lzma")
        || name_lc.ends_with(".tlz")
        || name_lc.ends_with(".tar.zst")
        || name_lc.ends_with(".tzst")
        || name_lc.ends_with(".tar.lz4")
        || name_lc.ends_with(".tar")
    {
        Box::new(TarBackend)
    } else if name_lc.ends_with(".rar") {
        Box::new(RarBackend)
    } else if name_lc.ends_with(".iso") {
        Box::new(IsoBackend)
    } else if name_lc.ends_with(".deb") {
        Box::new(DebBackend)
    } else if name_lc.ends_with(".gz")
        || name_lc.ends_with(".bz2")
        || name_lc.ends_with(".xz")
        || name_lc.ends_with(".lzma")
        || name_lc.ends_with(".zstd")
        || name_lc.ends_with(".zst")
        || name_lc.ends_with(".lz4")
    {
        Box::new(SingleFileBackend)
    } else {
        Box::new(UnsupportedBackend)
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
    /// True when the entry belongs to a password-protected archive that has not
    /// yet been unlocked (i.e. listed without a password).
    #[allow(dead_code)]
    pub is_encrypted: bool,
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
    list_archive_entries_with_backend(archive_path, prefix, password, None)
}

/// Same as `list_archive_entries`, but allows injecting a custom backend (for testing).
#[allow(dead_code)]
pub fn list_archive_entries_with_backend(
    archive_path: &Path,
    prefix: &str,
    password: Option<&str>,
    backend: Option<Box<dyn ArchiveBackend>>,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let backend = get_backend(archive_path, backend);
    backend.list_entries(archive_path, prefix, password)
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
                sort_ext: std::path::Path::new(&e.name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_default(),
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
    extract_entry_to_tempfile_with_backend(archive_path, inner_path, password, None)
}

/// Same as `extract_entry_to_tempfile`, but allows injecting a custom backend.
#[allow(dead_code)]
pub fn extract_entry_to_tempfile_with_backend(
    archive_path: &Path,
    inner_path: &str,
    password: Option<&str>,
    backend: Option<Box<dyn ArchiveBackend>>,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    let backend = get_backend(archive_path, backend);
    let data = backend.extract_entry_bytes(archive_path, inner_path, password)?;

    let suffix = Path::new(inner_path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| format!(".{n}"))
        .unwrap_or_default();

    let mut tmp = tempfile::Builder::new()
        .suffix(&suffix)
        .tempfile_in(flux_scratch_dir())
        .map_err(|e| ArchiveError::Other(format!("temp file: {e}")))?;

    tmp.write_all(&data)
        .map_err(|e| ArchiveError::Other(format!("write temp: {e}")))?;
    tmp.flush()
        .map_err(|e| ArchiveError::Other(format!("flush: {e}")))?;
    Ok(tmp)
}

/// Extracts a folder entry and all of its nested contents from an archive into a
/// temporary directory under `/tmp`.
///
/// Returns the `PathBuf` pointing to the extracted root folder.
#[allow(dead_code)]
pub fn extract_dir_to_tempdir(
    archive_path: &std::path::Path,
    inner_dir: &str,
    password: Option<&str>,
) -> Result<std::path::PathBuf, ArchiveError> {
    extract_dir_to_tempdir_with_backend(archive_path, inner_dir, password, None)
}

/// Same as `extract_dir_to_tempdir`, but allows injecting a custom backend.
#[allow(dead_code)]
pub fn extract_dir_to_tempdir_with_backend(
    archive_path: &std::path::Path,
    inner_dir: &str,
    password: Option<&str>,
    backend: Option<Box<dyn ArchiveBackend>>,
) -> Result<std::path::PathBuf, ArchiveError> {
    let backend = get_backend(archive_path, backend);
    backend.extract_dir(archive_path, inner_dir, password)
}

// ─── Helper to extract bytes via a temporary file ────────────────────────────

/// Generic helper that calls an extraction function that writes to a tempfile,
/// then reads the tempfile content into a `Vec<u8>`.
fn extract_bytes_via_tempfile<F>(
    archive_path: &Path,
    inner_path: &str,
    password: Option<&str>,
    extract_fn: F,
) -> Result<Vec<u8>, ArchiveError>
where
    F: FnOnce(
        &Path,
        &str,
        Option<&str>,
        tempfile::NamedTempFile,
    ) -> Result<tempfile::NamedTempFile, ArchiveError>,
{
    let suffix = Path::new(inner_path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| format!(".{n}"))
        .unwrap_or_default();

    let tmp = tempfile::Builder::new()
        .suffix(&suffix)
        .tempfile_in(flux_scratch_dir())
        .map_err(|e| ArchiveError::Other(format!("temp file: {e}")))?;

    let tmp = extract_fn(archive_path, inner_path, password, tmp)?;

    let mut data = Vec::new();
    let mut file = std::fs::File::open(tmp.path())
        .map_err(|e| ArchiveError::Other(format!("open temp: {e}")))?;
    std::io::copy(&mut file, &mut data)
        .map_err(|e| ArchiveError::Other(format!("read temp: {e}")))?;
    Ok(data)
}

// ─── The rest of the file (helpers, format-specific implementations) ────────
// (All existing helper functions remain exactly as they were, unchanged.)

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
        ArchiveError::Other(crate::i18n::tr(
            "RAR support requires 'unar' or 'unrar' (install via your package manager)",
        ))
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
        collect_entry(seen, raw_name, is_dir, size, 0, prefix, false);
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
        collect_entry(seen, &raw_name, is_dir, size, 0, prefix, false);
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

        collect_entry(&mut seen, &raw_name, is_dir, size, mtime, prefix, false);
    }

    Ok(seen.into_values().collect())
}

fn make_dest_dir(inner_dir: &str) -> Result<(tempfile::TempDir, PathBuf), ArchiveError> {
    let folder_name = Path::new(inner_dir)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "folder".to_string());

    let temp_dir = tempfile::Builder::new()
        .prefix(&format!(".tmp.{}.", folder_name))
        .tempdir_in(flux_scratch_dir())
        .map_err(|e| ArchiveError::Other(format!("tempdir creation failed: {e}")))?;

    let dest_dir = temp_dir.path().join(&folder_name);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| ArchiveError::Other(format!("create_dir_all failed: {e}")))?;

    Ok((temp_dir, dest_dir))
}

fn extract_dir_zip(
    archive_path: &Path,
    inner_dir: &str,
    password: Option<&str>,
) -> Result<PathBuf, ArchiveError> {
    let (temp_dir, dest_dir) = make_dest_dir(inner_dir)?;

    let prefix = format!("{}/", inner_dir.trim_end_matches('/'));

    let file = std::fs::File::open(archive_path)
        .map_err(|e| ArchiveError::Other(format!("open archive failed: {e}")))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| ArchiveError::Other(format!("ZIP parse failed: {e}")))?;

    for i in 0..archive.len() {
        let (name, is_dir, mut reader): (String, bool, Box<dyn Read>) = match password {
            Some(pwd) => match archive.by_index_decrypt(i, pwd.as_bytes()) {
                Ok(e) => {
                    let n = e.name().replace('\\', "/");
                    let d = e.is_dir();
                    (n, d, Box::new(e))
                }
                Err(zip::result::ZipError::InvalidPassword)
                | Err(zip::result::ZipError::UnsupportedArchive(_)) => {
                    return Err(ArchiveError::WrongPassword)
                }
                Err(e) => return Err(ArchiveError::Other(format!("ZIP idx {i}: {e}"))),
            },
            None => match archive.by_index(i) {
                Ok(e) => {
                    let n = e.name().replace('\\', "/");
                    let d = e.is_dir();
                    (n, d, Box::new(e))
                }
                Err(e) => return Err(ArchiveError::Other(format!("ZIP idx {i}: {e}"))),
            },
        };

        if !name.starts_with(&prefix) {
            continue;
        }

        let relative = name.trim_start_matches(prefix.as_str());
        if relative.is_empty() {
            continue;
        }

        let out_path = dest_dir.join(relative);
        if !out_path.starts_with(&dest_dir) {
            continue; // skip malicious entry silently
        }
        if is_dir {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| ArchiveError::Other(format!("create dir failed: {e}")))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ArchiveError::Other(format!("create parent dir failed: {e}")))?;
            }
            let mut outfile = std::fs::File::create(&out_path)
                .map_err(|e| ArchiveError::Other(format!("create file failed: {e}")))?;
            std::io::copy(&mut reader, &mut outfile)
                .map_err(|e| ArchiveError::Other(format!("copy entry failed: {e}")))?;
        }
    }

    let result = dest_dir.clone();
    register_temp_dir(temp_dir);
    Ok(result)
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

        collect_entry(
            &mut seen,
            raw_name_trimmed,
            is_dir,
            size,
            mtime,
            prefix,
            false,
        );
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

fn extract_dir_7z(
    archive_path: &Path,
    inner_dir: &str,
    password: Option<&str>,
) -> Result<PathBuf, ArchiveError> {
    let (temp_dir, dest_dir) = make_dest_dir(inner_dir)?;

    let prefix = format!("{}/", inner_dir.trim_end_matches('/'));

    let file =
        std::fs::File::open(archive_path).map_err(|e| ArchiveError::Other(format!("open: {e}")))?;
    let pwd: Password = password.map(Password::from).unwrap_or_else(Password::empty);
    let mut reader = ArchiveReader::new(file, pwd).map_err(|e| match e {
        sevenz_rust2::Error::PasswordRequired => ArchiveError::PasswordRequired,
        sevenz_rust2::Error::MaybeBadPassword(_) => {
            if password.is_none() {
                ArchiveError::PasswordRequired
            } else {
                ArchiveError::WrongPassword
            }
        }
        _ => ArchiveError::Other(e.to_string()),
    })?;

    reader
        .for_each_entries(
            &mut |entry: &sevenz_rust2::ArchiveEntry, r: &mut dyn Read| {
                let raw = entry.name().replace('\\', "/");
                if !raw.starts_with(&prefix) {
                    std::io::copy(r, &mut std::io::sink()).map_err(sevenz_rust2::Error::from)?;
                    return Ok(true);
                }
                let relative = raw.trim_start_matches(prefix.as_str());
                if relative.is_empty() {
                    std::io::copy(r, &mut std::io::sink()).map_err(sevenz_rust2::Error::from)?;
                    return Ok(true);
                }

                let out_path = dest_dir.join(relative);
                if entry.is_directory {
                    std::fs::create_dir_all(&out_path).ok();
                    std::io::copy(r, &mut std::io::sink()).map_err(sevenz_rust2::Error::from)?;
                } else {
                    if let Some(parent) = out_path.parent() {
                        std::fs::create_dir_all(parent).ok();
                    }
                    let mut outfile = std::fs::File::create(&out_path).map_err(|e| {
                        sevenz_rust2::Error::from(std::io::Error::other(e.to_string()))
                    })?;
                    std::io::copy(r, &mut outfile).map_err(sevenz_rust2::Error::from)?;
                }
                Ok(true)
            },
        )
        .map_err(|e| ArchiveError::Other(e.to_string()))?;

    let result = dest_dir.clone();
    register_temp_dir(temp_dir);
    Ok(result)
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
        let raw_name = raw_name.strip_prefix("./").unwrap_or(&raw_name);
        if raw_name.is_empty() {
            continue;
        }
        let raw_name = raw_name.trim_end_matches('/');
        let is_dir = header.entry_type().is_dir();
        let size = header.size().unwrap_or(0);
        let mtime = header.mtime().unwrap_or(0) as i64;
        collect_entry(&mut seen, raw_name, is_dir, size, mtime, prefix, false);
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
        let path = path.strip_prefix("./").unwrap_or(&path);
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

fn extract_dir_tar<R: Read>(reader: R, inner_dir: &str) -> Result<PathBuf, ArchiveError> {
    let (temp_dir, dest_dir) = make_dest_dir(inner_dir)?;

    let prefix = format!("{}/", inner_dir.trim_end_matches('/'));

    let mut archive = tar::Archive::new(reader);
    for entry in archive
        .entries()
        .map_err(|e| ArchiveError::Other(format!("TAR: {e}")))?
    {
        let mut entry = entry.map_err(|e| ArchiveError::Other(format!("TAR entry: {e}")))?;
        let raw_path = entry
            .path()
            .map_err(|e| ArchiveError::Other(format!("TAR path: {e}")))?
            .to_string_lossy()
            .replace('\\', "/");

        let raw_path = raw_path.strip_prefix("./").unwrap_or(&raw_path);

        if !raw_path.starts_with(&prefix) {
            continue;
        }
        let relative = raw_path.trim_start_matches(prefix.as_str());
        if relative.is_empty() {
            continue;
        }

        let out_path = dest_dir.join(relative);
        let is_dir = entry.header().entry_type().is_dir();

        if is_dir {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| ArchiveError::Other(format!("create dir failed: {e}")))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ArchiveError::Other(format!("create parent dir failed: {e}")))?;
            }
            let mut outfile = std::fs::File::create(&out_path)
                .map_err(|e| ArchiveError::Other(format!("create file failed: {e}")))?;
            std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| ArchiveError::Other(format!("copy entry failed: {e}")))?;
        }
    }

    let result = dest_dir.clone();
    register_temp_dir(temp_dir);
    Ok(result)
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
        is_encrypted: false,
    }])
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
    is_encrypted: bool,
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
            is_encrypted,
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

struct IsoFileDevice(std::fs::File);

impl iso9660_simple::Read for IsoFileDevice {
    fn read(&mut self, position: usize, buffer: &mut [u8]) -> Option<()> {
        use std::io::{Read, Seek, SeekFrom};
        if self.0.seek(SeekFrom::Start(position as u64)).is_err() {
            return None;
        }
        if self.0.read_exact(buffer).is_ok() {
            Some(())
        } else {
            None
        }
    }
}

/// Detects whether an ISO image contains a UDF filesystem by probing sector 256
/// for the UDF Anchor Volume Descriptor Pointer tag (tag ID 2, little-endian u16).
fn iso_has_udf(file: &mut std::fs::File) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    // UDF AVDP is always at sector 256 (2048-byte sectors).
    if file.seek(SeekFrom::Start(256 * 2048)).is_err() {
        return false;
    }
    let mut tag = [0u8; 2];
    if file.read_exact(&mut tag).is_err() {
        return false;
    }
    u16::from_le_bytes(tag) == 2
}

/// Parses `7z l -slt` machine-readable output into [`ArchiveEntry`] items,
/// filtering to immediate children of `prefix`.
fn parse_7z_iso_list(stdout: &str, prefix: &str, seen: &mut HashMap<String, ArchiveEntry>) {
    let pfx = if prefix.is_empty() {
        String::new()
    } else {
        format!("{}/", prefix.trim_end_matches('/'))
    };

    let mut cur_path = String::new();
    let mut cur_size: u64 = 0;
    let mut cur_is_dir = false;
    let mut cur_mtime: i64 = 0;
    let mut in_block = false;

    let flush = |seen: &mut HashMap<String, ArchiveEntry>,
                 path: &str,
                 is_dir: bool,
                 size: u64,
                 mtime: i64,
                 pfx: &str| {
        if path.is_empty() {
            return;
        }
        collect_entry(
            seen,
            path,
            is_dir,
            size,
            mtime,
            pfx.trim_end_matches('/'),
            false,
        );
    };

    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("----------") {
            in_block = true;
            continue;
        }
        if !in_block {
            continue;
        }
        if line.is_empty() {
            flush(seen, &cur_path, cur_is_dir, cur_size, cur_mtime, &pfx);
            cur_path.clear();
            cur_size = 0;
            cur_is_dir = false;
            cur_mtime = 0;
            continue;
        }
        if let Some(rest) = line.strip_prefix("Path = ") {
            // 7z on UDF ISOs emits both the ISO 9660 and UDF paths separated
            // by a newline with the same key, take the last assignment (UDF).
            cur_path = rest.replace('\\', "/");
        } else if let Some(rest) = line.strip_prefix("Size = ") {
            cur_size = rest.trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("Attributes = ") {
            cur_is_dir = rest.trim_start().starts_with('D');
        } else if let Some(rest) = line.strip_prefix("Modified = ") {
            // Format: "2022-09-17 12:34:56"
            cur_mtime = rest
                .trim()
                .split(' ')
                .next()
                .and_then(|date| {
                    let parts: Vec<u32> = date.split('-').filter_map(|p| p.parse().ok()).collect();
                    if parts.len() == 3 {
                        let y = parts[0] as i64;
                        let m = parts[1] as i64;
                        let d = parts[2] as i64;
                        Some((y - 1970) * 31_557_600 + m * 2_629_800 + d * 86_400)
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
        }
    }
    // Flush the last block (no trailing blank line).
    flush(seen, &cur_path, cur_is_dir, cur_size, cur_mtime, &pfx);
}

fn list_iso(archive_path: &Path, prefix: &str) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let mut file = std::fs::File::open(archive_path)
        .map_err(|e| ArchiveError::Other(format!("open iso: {e}")))?;

    if iso_has_udf(&mut file) {
        return list_iso_udf(archive_path, prefix);
    }

    // Pure ISO 9660 path.
    let mut iso = iso9660_simple::ISO9660::from_device(IsoFileDevice(file))
        .ok_or_else(|| ArchiveError::Other(crate::i18n::tr("failed to parse ISO image")))?;

    let root_lba = {
        let r = iso.root().lba.get() as usize;
        if r == 0 {
            16
        } else {
            r
        }
    };

    let mut seen = HashMap::new();

    fn walk_iso(
        iso: &mut iso9660_simple::ISO9660,
        lba: usize,
        current_path: &str,
        prefix: &str,
        seen: &mut HashMap<String, ArchiveEntry>,
    ) {
        let entries_owned: Vec<_> = iso.read_directory(lba).collect();

        for entry in entries_owned {
            let name = entry
                .name
                .trim_end_matches('.')
                .split(';')
                .next()
                .unwrap_or("")
                .to_string();
            if name.is_empty() || name == "\x00" || name == "\x01" {
                continue;
            }

            let inner_path = if current_path.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", current_path, name)
            };

            let is_dir = entry.is_folder();
            let size = entry.file_size() as u64;
            let child_lba = entry.record.lba.get() as usize;

            collect_entry(seen, &inner_path, is_dir, size, 0, prefix, false);

            if is_dir && child_lba != lba && child_lba != 0 {
                walk_iso(iso, child_lba, &inner_path, prefix, seen);
            }
        }
    }

    walk_iso(&mut iso, root_lba, "", prefix, &mut seen);
    Ok(seen.into_values().collect())
}

/// Lists a UDF ISO by shelling out to `7z l -slt`, mirroring the RAR strategy.
fn list_iso_udf(archive_path: &Path, prefix: &str) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let output = std::process::Command::new("7z")
        .args(["l", "-slt", "-so"])
        .arg(archive_path)
        .output()
        .map_err(|e| {
            ArchiveError::Other(format!(
                "{}: {e}",
                crate::i18n::tr("7z spawn error. Install p7zip-full (apt) or p7zip (pacman).")
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ArchiveError::Other(format!("7z failed: {}", stderr.trim())));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut seen = HashMap::new();
    parse_7z_iso_list(&stdout, prefix, &mut seen);
    Ok(seen.into_values().collect())
}

fn extract_iso(
    archive_path: &Path,
    inner_path: &str,
    mut tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    let mut file = std::fs::File::open(archive_path)
        .map_err(|e| ArchiveError::Other(format!("open iso: {e}")))?;

    if iso_has_udf(&mut file) {
        return extract_iso_udf(archive_path, inner_path, tmp);
    }

    let mut iso = iso9660_simple::ISO9660::from_device(IsoFileDevice(file))
        .ok_or_else(|| ArchiveError::Other(crate::i18n::tr("failed to parse ISO image")))?;

    let root_lba = iso.root().lba.get() as usize;
    let mut target_entry = None;

    fn find_entry(
        iso: &mut iso9660_simple::ISO9660,
        lba: usize,
        current_path: &str,
        target: &str,
        found: &mut Option<iso9660_simple::ISODirectoryEntry>,
    ) {
        let entries_owned: Vec<_> = iso.read_directory(lba).collect();

        for entry in entries_owned {
            // Mirror the normalisation applied in list_iso so paths match.
            let name = entry
                .name
                .trim_end_matches('.')
                .split(';')
                .next()
                .unwrap_or("")
                .to_string();
            if name.is_empty() || name == "\x00" || name == "\x01" {
                continue;
            }
            let inner = if current_path.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", current_path, name)
            };

            if inner == target {
                *found = Some(entry.clone());
                return;
            }

            if entry.is_folder() {
                let child_lba = entry.record.lba.get() as usize;
                if child_lba != lba && child_lba != 0 {
                    find_entry(iso, child_lba, &inner, target, found);
                    if found.is_some() {
                        return;
                    }
                }
            }
        }
    }

    find_entry(&mut iso, root_lba, "", inner_path, &mut target_entry);

    let entry = target_entry
        .ok_or_else(|| ArchiveError::Other(format!("not found in ISO: {inner_path}")))?;

    let size = entry.file_size();
    let mut buffer = vec![0u8; size as usize];
    iso.read_file(&entry, 0, &mut buffer)
        .ok_or_else(|| ArchiveError::Other("failed to read file from ISO".into()))?;

    tmp.write_all(&buffer)
        .map_err(|e| ArchiveError::Other(format!("write temp: {e}")))?;
    tmp.flush()
        .map_err(|e| ArchiveError::Other(format!("flush temp: {e}")))?;

    Ok(tmp)
}

/// Extracts a single file from a UDF ISO by shelling out to `7z e`.
fn extract_iso_udf(
    archive_path: &Path,
    inner_path: &str,
    mut tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    let tmp_dir = tempfile::tempdir().map_err(|e| ArchiveError::Other(format!("tempdir: {e}")))?;

    let file_name = Path::new(inner_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(inner_path);

    // `7z e` flattens paths, `-r` ensures it finds the file anywhere in the tree.
    let output = std::process::Command::new("7z")
        .args(["e", "-y"])
        .arg(archive_path)
        .arg(format!("-o{}", tmp_dir.path().display()))
        // Pass the full inner path so 7z targets the exact file.
        .arg(inner_path)
        .output()
        .map_err(|e| ArchiveError::Other(format!("7z spawn: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ArchiveError::Other(format!(
            "7z extract failed: {}",
            stderr.trim()
        )));
    }

    let extracted = find_file_recursive(tmp_dir.path(), file_name)?;

    let mut src = std::fs::File::open(&extracted)
        .map_err(|e| ArchiveError::Other(format!("open extracted: {e}")))?;
    std::io::copy(&mut src, &mut tmp).map_err(|e| ArchiveError::Other(format!("copy: {e}")))?;
    tmp.flush()
        .map_err(|e| ArchiveError::Other(format!("flush: {e}")))?;
    Ok(tmp)
}

// ─── .deb helpers ─────────────────────────────────────────────────────────────
fn extract_deb_data(deb_path: &Path) -> Result<(tempfile::TempDir, PathBuf), ArchiveError> {
    let tmp = tempfile::tempdir().map_err(|e| ArchiveError::Other(format!("tempdir: {e}")))?;

    // Extract all members, we care about data.tar.*
    let out = std::process::Command::new("ar")
        .args(["x", &deb_path.to_string_lossy()])
        .current_dir(tmp.path())
        .output()
        .map_err(|e| ArchiveError::Other(format!("ar spawn failed (install binutils): {e}")))?;

    if !out.status.success() {
        return Err(ArchiveError::Other(format!(
            "ar failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }

    // Find the data tarball (data.tar.gz / data.tar.xz / data.tar.zst / …)
    let data_tar = std::fs::read_dir(tmp.path())
        .map_err(|e| ArchiveError::Other(format!("readdir: {e}")))?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("data.tar"))
                .unwrap_or(false)
        })
        .ok_or_else(|| ArchiveError::Other("data.tar.* not found inside .deb".into()))?;

    Ok((tmp, data_tar))
}

fn list_deb(archive_path: &Path, prefix: &str) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let (_tmp, data_tar) = extract_deb_data(archive_path)?;
    TarBackend.list_entries(&data_tar, prefix, None)
}

fn extract_deb(
    archive_path: &Path,
    inner_path: &str,
    tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    let (_dir, data_tar) = extract_deb_data(archive_path)?;
    // Deb data tarballs store entries as "./usr/bin/foo" - prepend "./" if absent.
    let normalized = if inner_path.starts_with("./") {
        inner_path.to_owned()
    } else {
        format!("./{inner_path}")
    };
    let name_lc = lc_name(&data_tar);
    if name_lc.ends_with(".tar.gz") || name_lc.ends_with(".tgz") {
        extract_tar(
            flate2::read::GzDecoder::new(open_buf(&data_tar)?),
            &normalized,
            tmp,
        )
    } else if name_lc.ends_with(".tar.bz2") || name_lc.ends_with(".tbz2") {
        extract_tar(
            bzip2::read::BzDecoder::new(open_buf(&data_tar)?),
            &normalized,
            tmp,
        )
    } else if name_lc.ends_with(".tar.xz")
        || name_lc.ends_with(".txz")
        || name_lc.ends_with(".tar.lzma")
        || name_lc.ends_with(".tlz")
    {
        extract_tar(
            xz2::read::XzDecoder::new(open_buf(&data_tar)?),
            &normalized,
            tmp,
        )
    } else if name_lc.ends_with(".tar.zst") || name_lc.ends_with(".tzst") {
        let dec = zstd::stream::read::Decoder::new(open_buf(&data_tar)?)
            .map_err(|e| ArchiveError::Other(format!("zstd: {e}")))?;
        extract_tar(dec, &normalized, tmp)
    } else if name_lc.ends_with(".tar.lz4") {
        extract_tar(
            lz4_flex::frame::FrameDecoder::new(open_buf(&data_tar)?),
            &normalized,
            tmp,
        )
    } else {
        extract_tar(open_buf(&data_tar)?, &normalized, tmp)
    }
}

fn extract_dir_deb(archive_path: &Path, inner_dir: &str) -> Result<PathBuf, ArchiveError> {
    let (_dir, data_tar) = extract_deb_data(archive_path)?;
    let name_lc = lc_name(&data_tar);
    if name_lc.ends_with(".tar.gz") || name_lc.ends_with(".tgz") {
        extract_dir_tar(
            flate2::read::GzDecoder::new(open_buf(&data_tar)?),
            inner_dir,
        )
    } else if name_lc.ends_with(".tar.bz2") || name_lc.ends_with(".tbz2") {
        extract_dir_tar(bzip2::read::BzDecoder::new(open_buf(&data_tar)?), inner_dir)
    } else if name_lc.ends_with(".tar.xz")
        || name_lc.ends_with(".txz")
        || name_lc.ends_with(".tar.lzma")
        || name_lc.ends_with(".tlz")
    {
        extract_dir_tar(xz2::read::XzDecoder::new(open_buf(&data_tar)?), inner_dir)
    } else if name_lc.ends_with(".tar.zst") || name_lc.ends_with(".tzst") {
        let dec = zstd::stream::read::Decoder::new(open_buf(&data_tar)?)
            .map_err(|e| ArchiveError::Other(format!("zstd: {e}")))?;
        extract_dir_tar(dec, inner_dir)
    } else if name_lc.ends_with(".tar.lz4") {
        extract_dir_tar(
            lz4_flex::frame::FrameDecoder::new(open_buf(&data_tar)?),
            inner_dir,
        )
    } else {
        extract_dir_tar(open_buf(&data_tar)?, inner_dir)
    }
}
