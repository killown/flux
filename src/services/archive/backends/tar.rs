//! TAR backend with all compression flavours.

use crate::services::archive::entry::collect_entry;
use crate::services::archive::util::{
    extract_bytes_via_tempfile, lc_name, make_dest_dir_at, open_buf,
};
use crate::services::archive::{register_temp_dir, ArchiveBackend, ArchiveEntry, ArchiveError};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub(in crate::services::archive) struct TarBackend;

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
        let entry_path = entry
            .path()
            .map_err(|e| ArchiveError::Other(format!("TAR path: {e}")))?;
        let stripped_path = crate::utils::strip_current_dir(&entry_path);
        let raw_name = stripped_path.to_string_lossy().replace('\\', "/");
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

pub(super) fn extract_tar<R: Read>(
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
        let entry_path = entry
            .path()
            .map_err(|e| ArchiveError::Other(format!("TAR path: {e}")))?;
        let stripped_path = crate::utils::strip_current_dir(&entry_path);
        let path = stripped_path.to_string_lossy().replace('\\', "/");
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

pub(super) fn extract_dir_tar<R: Read>(
    reader: R,
    inner_dir: &str,
) -> Result<PathBuf, ArchiveError> {
    extract_dir_tar_at(reader, inner_dir, None)
}

pub(in crate::services::archive) fn extract_dir_tar_at<R: Read>(
    reader: R,
    inner_dir: &str,
    override_dest: Option<&Path>,
) -> Result<PathBuf, ArchiveError> {
    let (temp_dir, dest_dir) = make_dest_dir_at(inner_dir, override_dest)?;

    let prefix = if inner_dir.is_empty() {
        String::new()
    } else {
        format!("{}/", inner_dir.trim_matches('/'))
    };

    let mut archive = tar::Archive::new(reader);
    for entry in archive
        .entries()
        .map_err(|e| ArchiveError::Other(format!("TAR: {e}")))?
    {
        let mut entry = entry.map_err(|e| ArchiveError::Other(format!("TAR entry: {e}")))?;
        let entry_path = entry
            .path()
            .map_err(|e| ArchiveError::Other(format!("TAR path: {e}")))?;
        let stripped_path = crate::utils::strip_current_dir(&entry_path);
        let raw_path = stripped_path.to_string_lossy().replace('\\', "/");

        if !raw_path.starts_with(&prefix) {
            continue;
        }
        let relative = raw_path.trim_start_matches(prefix.as_str());
        if relative.is_empty() {
            continue;
        }

        let rel_path = Path::new(relative);
        let has_traversal = rel_path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        });

        let out_path = dest_dir.join(rel_path);
        if has_traversal || !out_path.starts_with(&dest_dir) {
            continue;
        }

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
