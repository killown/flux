//! Helpers shared by several archive backends.

use crate::services::archive::{flux_scratch_dir, ArchiveError};
use std::io::BufReader;
use std::path::{Path, PathBuf};

// ─── Shared helpers ───────────────────────────────────────────────────────────

pub(super) fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
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

/// Walks `dir` recursively to find the first file whose name matches `file_name`.
pub(super) fn find_file_recursive(dir: &Path, file_name: &str) -> Result<PathBuf, ArchiveError> {
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

pub(super) fn make_dest_dir(inner_dir: &str) -> Result<(tempfile::TempDir, PathBuf), ArchiveError> {
    make_dest_dir_at(inner_dir, None)
}

pub(super) fn make_dest_dir_at(
    inner_dir: &str,
    override_dest: Option<&Path>,
) -> Result<(tempfile::TempDir, PathBuf), ArchiveError> {
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
