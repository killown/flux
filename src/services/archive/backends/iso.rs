//! ISO 9660 and UDF backend.

use crate::services::archive::entry::collect_entry;
use crate::services::archive::util::{extract_bytes_via_tempfile, find_file_recursive};
use crate::services::archive::{ArchiveBackend, ArchiveEntry, ArchiveError};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(in crate::services::archive) struct IsoBackend;

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
