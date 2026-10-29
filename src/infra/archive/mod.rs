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

mod backend;
mod backends;
mod entry;
mod error;
mod format;
mod lock_hint;
mod temp;
pub mod thumb_cache;
mod uri;
mod util;

pub use backend::{get_backend, ArchiveBackend};
#[allow(unused_imports)]
pub use backends::{read_cached_thumbnail, write_thumbnails_to_archive};
pub use entry::{entries_to_load_contexts, ArchiveEntry};
pub use error::ArchiveError;
#[allow(unused_imports)]
pub use format::{is_browsable_archive, is_supported_archive, BROWSABLE_ARCHIVE_EXTENSIONS};
#[allow(unused_imports)]
pub use lock_hint::{zip_is_encrypted, ArchiveLockSnapshot};
#[allow(unused_imports)]
pub use temp::{
    clear_archive_session_temp, flux_scratch_dir, purge_stale_scratch_dirs, register_temp_dir,
    register_temp_file,
};
#[allow(unused_imports)]
pub use uri::{
    build_archive_uri, decode_archive_host, encode_archive_host,
    extract_archive_entry_for_playback, get_archive_thumbnail, parse_archive_uri, ARCHIVE_URI,
};

use backends::{extract_dir_7z_at, extract_dir_tar_at, extract_dir_zip_at};
use std::io::Write;
use std::path::Path;
use util::{copy_dir_recursive, lc_name, open_buf};

/// Extracts the entire archive directly into `dest_dir` (which must already exist).
///
/// This skips the internal temp-dir + rename dance, so
/// the caller can poll `dest_dir` while extraction is in progress and get real
/// byte counts.
pub fn extract_archive_to_dir(
    archive_path: &Path,
    dest_dir: &Path,
    password: Option<&str>,
) -> Result<(), ArchiveError> {
    crate::hit!("extract_archive_to_dir");
    let name_lc = lc_name(archive_path);

    if name_lc.ends_with(".zip") {
        extract_dir_zip_at(archive_path, "", password, Some(dest_dir))?;
    } else if name_lc.ends_with(".7z") {
        extract_dir_7z_at(archive_path, "", password, Some(dest_dir))?;
    } else if name_lc.ends_with(".tar.gz") || name_lc.ends_with(".tgz") {
        extract_dir_tar_at(
            flate2::read::GzDecoder::new(open_buf(archive_path)?),
            "",
            Some(dest_dir),
        )?;
    } else if name_lc.ends_with(".tar.bz2") || name_lc.ends_with(".tbz2") {
        extract_dir_tar_at(
            bzip2::read::BzDecoder::new(open_buf(archive_path)?),
            "",
            Some(dest_dir),
        )?;
    } else if name_lc.ends_with(".tar.xz")
        || name_lc.ends_with(".txz")
        || name_lc.ends_with(".tar.lzma")
        || name_lc.ends_with(".tlz")
    {
        extract_dir_tar_at(
            xz2::read::XzDecoder::new(open_buf(archive_path)?),
            "",
            Some(dest_dir),
        )?;
    } else if name_lc.ends_with(".tar.zst") || name_lc.ends_with(".tzst") {
        let dec = zstd::stream::read::Decoder::new(open_buf(archive_path)?)
            .map_err(|e| ArchiveError::Other(format!("zstd: {e}")))?;
        extract_dir_tar_at(dec, "", Some(dest_dir))?;
    } else if name_lc.ends_with(".tar.lz4") {
        extract_dir_tar_at(
            lz4_flex::frame::FrameDecoder::new(open_buf(archive_path)?),
            "",
            Some(dest_dir),
        )?;
    } else if name_lc.ends_with(".tar") {
        extract_dir_tar_at(open_buf(archive_path)?, "", Some(dest_dir))?;
    } else {
        // Formats without direct extraction support fall back to temporary directory extraction.
        let tmp_path = get_backend(archive_path, None).extract_dir(archive_path, "", password)?;
        for entry in std::fs::read_dir(&tmp_path)
            .map_err(|e| ArchiveError::Other(format!("readdir: {e}")))?
            .flatten()
        {
            let src = entry.path();
            let dst = dest_dir.join(entry.file_name());
            if std::fs::rename(&src, &dst).is_err() {
                if src.is_dir() {
                    copy_dir_recursive(&src, &dst).and_then(|_| std::fs::remove_dir_all(&src))
                } else {
                    std::fs::copy(&src, &dst)
                        .map(|_| ())
                        .and_then(|_| std::fs::remove_file(&src))
                }
                .map_err(|e| ArchiveError::Other(format!("move to dest: {e}")))?;
            }
        }
    }
    Ok(())
}

/// Returns the total uncompressed byte size of all file entries in the archive,
/// including those in nested folders.
pub fn archive_total_bytes(archive_path: &Path, password: Option<&str>) -> u64 {
    get_backend(archive_path, None)
        .total_bytes(archive_path, password)
        .unwrap_or(0)
}

// ─── Public entry point ───────────────────────────────────────────────────────

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
    crate::hit!("extract_entry_to_tempfile_with_backend");
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

/// Extracts one file entry into `dest_dir` as a regular file and returns its path.
pub fn extract_entry_to_unique_file(
    archive_path: &Path,
    inner_path: &str,
    password: Option<&str>,
    dest_dir: &Path,
) -> Result<std::path::PathBuf, ArchiveError> {
    crate::hit!("extract_entry_to_unique_file");
    let data =
        get_backend(archive_path, None).extract_entry_bytes(archive_path, inner_path, password)?;

    let name = Path::new(inner_path)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| ArchiveError::Other(format!("invalid entry name: {inner_path}")))?;

    // Split the base name from the extension, keeping `.tar.*` intact.
    let name_lc = name.to_ascii_lowercase();
    let split = name_lc
        .rfind(".tar.")
        .or_else(|| name.rfind('.').filter(|&i| i > 0))
        .unwrap_or(name.len());
    let (stem, ext) = name.split_at(split);

    let mut counter = 1u32;
    loop {
        let candidate = if counter == 1 {
            name.to_string()
        } else {
            format!("{stem}_{counter}{ext}")
        };
        let dest = dest_dir.join(candidate);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&dest)
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(&data).and_then(|_| file.flush()) {
                    drop(file);
                    let _ = std::fs::remove_file(&dest);
                    return Err(ArchiveError::Other(format!(
                        "write {}: {e}",
                        dest.display()
                    )));
                }
                return Ok(dest);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => counter += 1,
            Err(e) => {
                return Err(ArchiveError::Other(format!(
                    "create {}: {e}",
                    dest.display()
                )))
            }
        }
    }
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

/// Runs `f` holding the write lock of `archive_path`, so rewrites of one archive never overlap.
fn with_archive_write_lock<T>(archive_path: &Path, f: impl FnOnce() -> T) -> T {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, OnceLock};

    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let key = archive_path
        .canonicalize()
        .unwrap_or_else(|_| archive_path.to_path_buf());
    let lock = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(key)
        .or_default()
        .clone();
    let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    f()
}

/// Deletes an inner path entry from an archive file on disk.
#[allow(dead_code)]
pub fn remove_archive_entry(
    archive_path: &Path,
    inner_path: &str,
    password: Option<&str>,
    progress_cb: Option<&mut dyn FnMut(u64)>,
) -> Result<(), ArchiveError> {
    let backend = get_backend(archive_path, None);
    with_archive_write_lock(archive_path, || {
        backend.remove_entry(archive_path, inner_path, password, progress_cb)
    })
}

/// Adds an entry to an archive from a local file, failing if the name is taken.
#[allow(dead_code)]
pub fn write_archive_entry(
    archive_path: &Path,
    source_file: &Path,
    inner_path: &str,
    password: Option<&str>,
    progress_cb: Option<&mut dyn FnMut(u64)>,
) -> Result<(), ArchiveError> {
    let backend = get_backend(archive_path, None);
    with_archive_write_lock(archive_path, || {
        backend.write_entry(archive_path, source_file, inner_path, password, progress_cb)
    })
}

/// Creates an empty directory entry inside an archive file.
#[allow(dead_code)]
pub fn create_archive_directory(
    archive_path: &Path,
    inner_path: &str,
    password: Option<&str>,
) -> Result<(), ArchiveError> {
    let backend = get_backend(archive_path, None);
    with_archive_write_lock(archive_path, || {
        backend.create_directory(archive_path, inner_path, password)
    })
}
