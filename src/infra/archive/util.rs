//! Helpers shared by several archive backends.

use crate::domain::model::ArchiveError;
use crate::infra::archive::flux_scratch_dir;
use std::io::{BufReader, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

// ─── Shared helpers ───────────────────────────────────────────────────────────

pub(super) fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    crate::hit!("copy_dir_recursive");
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let d = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(entry.path())?, d)?;
        } else if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &d)?;
        } else {
            std::fs::copy(entry.path(), d)?;
        }
    }
    Ok(())
}

/// Upper bound for a single archive entry that is extracted to a scratch file
/// or held in memory.
pub(super) const MAX_ENTRY_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Upper bound for the total bytes one directory extraction may write.
pub(super) const MAX_EXTRACT_BYTES: u64 = 32 * 1024 * 1024 * 1024;

/// Copies `reader` into `writer`, failing once more than `limit` bytes are available.
pub(super) fn copy_limited<R: Read + ?Sized, W: Write + ?Sized>(
    reader: &mut R,
    writer: &mut W,
    limit: u64,
) -> std::io::Result<u64> {
    crate::hit!("copy_limited");
    let copied = std::io::copy(&mut (&mut *reader).take(limit), writer)?;
    if copied == limit {
        let mut probe = [0u8; 1];
        if reader.read(&mut probe)? != 0 {
            return Err(std::io::Error::other(format!(
                "entry exceeds the {limit} byte extraction limit"
            )));
        }
    }
    Ok(copied)
}

/// Rejects entry names that an external extractor would read as an option
/// (`-x`), a list file (`@list`) or a wildcard pattern instead of a literal path.
pub(super) fn reject_tool_arg(name: &str) -> Result<(), ArchiveError> {
    if name.starts_with('-') || name.starts_with('@') || name.contains(['*', '?']) {
        return Err(ArchiveError::Other(format!(
            "entry name not supported by the external extractor: {name}"
        )));
    }
    Ok(())
}

/// Opens `path` for reading without following a symlink at the final component.
pub(super) fn open_no_follow(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

/// Creates or truncates `path` for writing. A symlink already at `path` is
/// replaced and never followed.
pub(super) fn create_no_follow(path: &Path) -> std::io::Result<std::fs::File> {
    crate::hit!("create_no_follow");
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        std::fs::remove_file(path)?;
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

// ─── Helper to extract bytes via a temporary file ────────────────────────────

/// Generic helper that calls an extraction function that writes to a tempfile,
/// then reads the tempfile content into a `Vec<u8>`.
pub(super) fn extract_bytes_via_tempfile<F>(
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
    crate::hit!("extract_bytes_via_tempfile");
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
    copy_limited(&mut file, &mut data, MAX_ENTRY_BYTES)
        .map_err(|e| ArchiveError::Other(format!("read temp: {e}")))?;
    Ok(data)
}

/// Walks `dir` recursively to find the first file whose name matches `file_name`.
///
/// Symlinks are neither followed nor returned, so an extractor that unpacks a
/// link cannot point the result at a file outside `dir`.
pub(super) fn find_file_recursive(dir: &Path, file_name: &str) -> Result<PathBuf, ArchiveError> {
    crate::hit!("find_file_recursive");
    for entry in std::fs::read_dir(dir).map_err(|e| ArchiveError::Other(format!("readdir: {e}")))? {
        let entry = entry.map_err(|e| ArchiveError::Other(format!("entry: {e}")))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| ArchiveError::Other(format!("file type: {e}")))?;
        if file_type.is_dir() {
            if let Ok(found) = find_file_recursive(&path, file_name) {
                return Ok(found);
            }
        } else if file_type.is_file()
            && path.file_name().and_then(|n| n.to_str()) == Some(file_name)
        {
            return Ok(path);
        }
    }
    Err(ArchiveError::Other(format!(
        "extracted file not found: {file_name}"
    )))
}

pub(super) fn make_dest_dir(inner_dir: &str) -> Result<(tempfile::TempDir, PathBuf), ArchiveError> {
    make_dest_dir_at(inner_dir, None)
}

/// Joins an archive entry name onto `base`, or `None` if it could escape `base`.
pub(super) fn safe_join(base: &Path, relative: &str) -> Option<PathBuf> {
    crate::hit!("safe_join");
    let rel = Path::new(relative);
    let escapes = rel.components().any(|c| {
        matches!(
            c,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    });
    (!escapes).then(|| base.join(rel))
}

pub(super) fn make_dest_dir_at(
    inner_dir: &str,
    override_dest: Option<&Path>,
) -> Result<(tempfile::TempDir, PathBuf), ArchiveError> {
    crate::hit!("make_dest_dir_at");
    if let Some(dest) = override_dest {
        std::fs::create_dir_all(dest)
            .map_err(|e| ArchiveError::Other(format!("create_dir_all failed: {e}")))?;
        let dummy = tempfile::Builder::new()
            .prefix(".flux-dummy.")
            .tempdir_in(flux_scratch_dir())
            .map_err(|e| ArchiveError::Other(format!("dummy tempdir: {e}")))?;
        return Ok((dummy, dest.to_path_buf()));
    }

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

#[inline]
pub(super) fn open_buf(path: &Path) -> Result<BufReader<std::fs::File>, ArchiveError> {
    std::fs::File::open(path)
        .map(BufReader::new)
        .map_err(|e| ArchiveError::Other(format!("open: {e}")))
}

#[inline]
pub(super) fn lc_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

#[inline]
pub(super) fn strip_ext<'a>(name: &'a str, ext: &str) -> &'a str {
    name.strip_suffix(ext).unwrap_or(name)
}
