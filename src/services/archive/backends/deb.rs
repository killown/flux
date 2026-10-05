//! Debian package backend.

use super::tar::{extract_dir_tar, extract_tar, TarBackend};
use crate::services::archive::util::{extract_bytes_via_tempfile, lc_name, open_buf};
use crate::services::archive::{ArchiveBackend, ArchiveEntry, ArchiveError};
use std::path::{Path, PathBuf};

pub(in crate::services::archive) struct DebBackend; // for .deb (ar + inner data.tar.*)

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
    let inner_p = Path::new(inner_path);
    let stripped = crate::utils::strip_current_dir(inner_p);
    let normalized = format!("./{}", stripped.display());
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
