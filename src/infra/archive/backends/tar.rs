//! TAR backend with all compression flavours.

use crate::infra::archive::entry::collect_entry;
use crate::infra::archive::util::{
    copy_limited, extract_bytes_via_tempfile, lc_name, make_dest_dir_at, open_buf, MAX_ENTRY_BYTES,
    MAX_EXTRACT_BYTES,
};
use crate::infra::archive::{register_temp_dir, ArchiveBackend, ArchiveEntry, ArchiveError};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub(in crate::infra::archive) struct TarBackend;

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

    fn total_bytes(
        &self,
        archive_path: &Path,
        _password: Option<&str>,
    ) -> Result<u64, ArchiveError> {
        let name_lc = lc_name(archive_path);
        if name_lc.ends_with(".tar.gz") || name_lc.ends_with(".tgz") {
            total_tar(flate2::read::GzDecoder::new(open_buf(archive_path)?))
        } else if name_lc.ends_with(".tar.bz2") || name_lc.ends_with(".tbz2") {
            total_tar(bzip2::read::BzDecoder::new(open_buf(archive_path)?))
        } else if name_lc.ends_with(".tar.xz")
            || name_lc.ends_with(".txz")
            || name_lc.ends_with(".tar.lzma")
            || name_lc.ends_with(".tlz")
        {
            total_tar(xz2::read::XzDecoder::new(open_buf(archive_path)?))
        } else if name_lc.ends_with(".tar.zst") || name_lc.ends_with(".tzst") {
            let dec = zstd::stream::read::Decoder::new(open_buf(archive_path)?)
                .map_err(|e| ArchiveError::Other(format!("zstd open: {e}")))?;
            total_tar(dec)
        } else if name_lc.ends_with(".tar.lz4") {
            total_tar(lz4_flex::frame::FrameDecoder::new(open_buf(archive_path)?))
        } else {
            total_tar(open_buf(archive_path)?)
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
    crate::hit!("list_tar");
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

/// Sums the header sizes of regular files in a single pass.
fn total_tar<R: Read>(reader: R) -> Result<u64, ArchiveError> {
    crate::hit!("total_tar");
    let mut archive = tar::Archive::new(reader);
    let mut total = 0u64;
    for entry in archive
        .entries()
        .map_err(|e| ArchiveError::Other(format!("TAR: {e}")))?
    {
        let entry = entry.map_err(|e| ArchiveError::Other(format!("TAR entry: {e}")))?;
        let header = entry.header();
        if header.entry_type().is_file() {
            total = total.saturating_add(header.size().unwrap_or(0));
        }
    }
    Ok(total)
}

pub(super) fn extract_tar<R: Read>(
    reader: R,
    inner_path: &str,
    mut tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    crate::hit!("extract_tar");
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
            copy_limited(&mut entry, &mut tmp, MAX_ENTRY_BYTES)
                .map_err(|e| ArchiveError::Other(format!("copy: {e}")))?;
            tmp.flush()
                .map_err(|e| ArchiveError::Other(format!("flush: {e}")))?;
            return Ok(tmp);
        }
    }
    Err(ArchiveError::Other(format!("not found: {inner_path}")))
}

/// True when no component of `dir` below `base` is a symlink.
///
/// Extraction can run into an existing folder (`override_dest`) that already
/// holds symlinks, and an earlier symlink entry of the same archive creates
/// one too. Writing through either would land outside the destination.
/// Components that do not exist yet are fine: they are created as real
/// directories.
fn real_dirs_below(base: &Path, dir: &Path) -> bool {
    let Ok(rel) = dir.strip_prefix(base) else {
        return false;
    };
    let mut cur = base.to_path_buf();
    for comp in rel.components() {
        cur.push(comp);
        if std::fs::symlink_metadata(&cur).is_ok_and(|m| m.file_type().is_symlink()) {
            return false;
        }
    }
    true
}

/// Cached form of [`real_dirs_below`] for the per-entry hot path. Only
/// directories already verified as symlink-free are cached, and extraction
/// never turns one of them into a symlink: an existing directory is never
/// replaced by an entry.
fn dir_is_safe(base: &Path, dir: &Path, verified: &mut HashSet<PathBuf>) -> bool {
    if verified.contains(dir) {
        return true;
    }
    let safe = real_dirs_below(base, dir);
    if safe {
        verified.insert(dir.to_path_buf());
    }
    safe
}

/// Removes whatever non-directory sits at `path` so a link can take its
/// place. Returns `false` when a directory is in the way.
fn clear_non_dir(path: &Path) -> std::io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => Ok(false),
        Ok(_) => std::fs::remove_file(path).map(|()| true),
        Err(_) => Ok(true),
    }
}

pub(super) fn extract_dir_tar<R: Read>(
    reader: R,
    inner_dir: &str,
) -> Result<PathBuf, ArchiveError> {
    extract_dir_tar_at(reader, inner_dir, None)
}

pub(in crate::infra::archive) fn extract_dir_tar_at<R: Read>(
    reader: R,
    inner_dir: &str,
    override_dest: Option<&Path>,
) -> Result<PathBuf, ArchiveError> {
    crate::hit!("extract_dir_tar_at");
    let (temp_dir, dest_dir) = make_dest_dir_at(inner_dir, override_dest)?;
    let mut budget = MAX_EXTRACT_BYTES;

    let prefix = if inner_dir.is_empty() {
        String::new()
    } else {
        format!("{}/", inner_dir.trim_matches('/'))
    };

    let mut verified: HashSet<PathBuf> = HashSet::new();
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

        // `strip_prefix` removes the prefix exactly once. Trimming it
        // repeatedly would turn `a/a/x` into `x` when extracting `a`.
        let Some(relative) = raw_path.strip_prefix(prefix.as_str()) else {
            continue;
        };
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

        let entry_type = entry.header().entry_type();
        let is_dir = entry_type.is_dir();

        // Directory entries must themselves be real directories, every other
        // entry needs a symlink-free parent chain.
        let guard_dir = if is_dir {
            Some(out_path.as_path())
        } else {
            out_path.parent()
        };
        if !guard_dir.is_some_and(|d| dir_is_safe(&dest_dir, d, &mut verified)) {
            eprintln!("[archive] skipped TAR entry behind a symlink: {relative}");
            continue;
        }

        if is_dir {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| ArchiveError::Other(format!("create dir failed: {e}")))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ArchiveError::Other(format!("create parent dir failed: {e}")))?;
        }

        if matches!(
            entry_type,
            tar::EntryType::Regular | tar::EntryType::Continuous | tar::EntryType::GNUSparse
        ) {
            // An existing symlink at the target is replaced, never followed.
            // `O_NOFOLLOW` closes the window between this check and the open.
            if std::fs::symlink_metadata(&out_path).is_ok_and(|m| m.file_type().is_symlink()) {
                std::fs::remove_file(&out_path)
                    .map_err(|e| ArchiveError::Other(format!("replace symlink failed: {e}")))?;
            }
            let mut outfile = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&out_path)
                .map_err(|e| ArchiveError::Other(format!("create file failed: {e}")))?;
            budget -= copy_limited(&mut entry, &mut outfile, budget)
                .map_err(|e| ArchiveError::Other(format!("copy entry failed: {e}")))?;
        } else if entry_type.is_symlink() {
            let Some(target) = entry
                .link_name()
                .map_err(|e| ArchiveError::Other(format!("TAR link name: {e}")))?
                .map(|t| t.into_owned())
            else {
                eprintln!("[archive] skipped TAR symlink without a target: {relative}");
                continue;
            };
            let free = clear_non_dir(&out_path)
                .map_err(|e| ArchiveError::Other(format!("replace entry failed: {e}")))?;
            if !free {
                eprintln!("[archive] skipped TAR symlink over a directory: {relative}");
                continue;
            }
            // The target is stored as is. Writes through it are refused by
            // the symlink check above, so it cannot be used to escape.
            std::os::unix::fs::symlink(&target, &out_path)
                .map_err(|e| ArchiveError::Other(format!("create symlink failed: {e}")))?;
        } else if entry_type.is_hard_link() {
            let Some(link) = entry
                .link_name()
                .map_err(|e| ArchiveError::Other(format!("TAR link name: {e}")))?
                .map(|t| t.into_owned())
            else {
                eprintln!("[archive] skipped TAR hard link without a target: {relative}");
                continue;
            };
            // Hard link targets are archive paths, so they get the same
            // prefix and traversal handling as entry names.
            let link_raw = crate::utils::strip_current_dir(&link)
                .to_string_lossy()
                .replace('\\', "/");
            let source = link_raw
                .strip_prefix(prefix.as_str())
                .map(Path::new)
                .filter(|rel| {
                    !rel.components().any(|c| {
                        matches!(
                            c,
                            std::path::Component::ParentDir
                                | std::path::Component::RootDir
                                | std::path::Component::Prefix(_)
                        )
                    })
                })
                .map(|rel| dest_dir.join(rel));
            let linked = match source {
                Some(src)
                    if src
                        .parent()
                        .is_some_and(|d| dir_is_safe(&dest_dir, d, &mut verified)) =>
                {
                    clear_non_dir(&out_path).is_ok_and(|free| free)
                        && std::fs::hard_link(&src, &out_path).is_ok()
                }
                _ => false,
            };
            if !linked {
                eprintln!("[archive] skipped TAR hard link, target not extracted: {relative}");
            }
        } else if !matches!(
            entry_type,
            tar::EntryType::XGlobalHeader
                | tar::EntryType::XHeader
                | tar::EntryType::GNULongName
                | tar::EntryType::GNULongLink
        ) {
            // Devices and FIFOs are not recreated. Writing an empty regular
            // file in their place would change the data silently.
            eprintln!("[archive] skipped special TAR entry: {relative}");
        }
    }

    let result = dest_dir.clone();
    register_temp_dir(temp_dir);
    Ok(result)
}
