//! ZIP backend.

use crate::services::archive::entry::collect_entry;
use crate::services::archive::util::{extract_bytes_via_tempfile, make_dest_dir_at};
use crate::services::archive::{register_temp_dir, ArchiveBackend, ArchiveEntry, ArchiveError};
use std::collections::HashMap;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

pub(in crate::services::archive) struct ZipBackend;

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

    fn remove_entry(
        &self,
        archive_path: &Path,
        inner_path: &str,
        _password: Option<&str>,
        mut progress_cb: Option<&mut dyn FnMut(u64)>,
    ) -> Result<(), ArchiveError> {
        let src_file = std::fs::File::open(archive_path)
            .map_err(|e| ArchiveError::Other(format!("open archive: {e}")))?;
        let mut zip_in = zip::ZipArchive::new(src_file)
            .map_err(|e| ArchiveError::Other(format!("read zip: {e}")))?;

        let parent_dir = archive_path.parent().unwrap_or_else(|| Path::new("."));
        let mut tmp = tempfile::Builder::new()
            .prefix(".flux-update-")
            .suffix(".tmp")
            .tempfile_in(parent_dir)
            .map_err(|e| ArchiveError::Other(format!("create tempfile: {e}")))?;

        let target = inner_path.trim_start_matches('/');
        let target_dir_prefix = format!("{}/", target);

        {
            let mut zip_out = zip::ZipWriter::new(&mut tmp);

            for i in 0..zip_in.len() {
                let raw_entry = zip_in
                    .by_index_raw(i)
                    .map_err(|e| ArchiveError::Other(format!("read raw entry {i}: {e}")))?;
                let name = raw_entry.name().replace('\\', "/");
                let entry_size = raw_entry.compressed_size();

                // Skip the entry being deleted (or any child if deleting a directory)
                if name == target || name.starts_with(&target_dir_prefix) {
                    continue;
                }

                zip_out
                    .raw_copy_file(raw_entry)
                    .map_err(|e| ArchiveError::Other(format!("copy entry {name}: {e}")))?;

                if let Some(ref mut cb) = progress_cb {
                    cb(entry_size);
                }
            }

            zip_out
                .finish()
                .map_err(|e| ArchiveError::Other(format!("finalize zip: {e}")))?;
        }

        tmp.persist(archive_path)
            .map_err(|e| ArchiveError::Other(format!("replace original archive: {e}")))?;

        Ok(())
    }

    fn create_directory(
        &self,
        archive_path: &Path,
        inner_path: &str,
        _password: Option<&str>,
    ) -> Result<(), ArchiveError> {
        let src_file = std::fs::File::open(archive_path)
            .map_err(|e| ArchiveError::Other(format!("open archive: {e}")))?;
        let mut zip_in = zip::ZipArchive::new(src_file)
            .map_err(|e| ArchiveError::Other(format!("read zip: {e}")))?;

        let parent_dir = archive_path.parent().unwrap_or_else(|| Path::new("."));
        let mut tmp = tempfile::Builder::new()
            .prefix(".flux-update-")
            .suffix(".tmp")
            .tempfile_in(parent_dir)
            .map_err(|e| ArchiveError::Other(format!("create tempfile: {e}")))?;

        // Normalise: strip leading slash, ensure trailing slash
        let target = inner_path.trim_start_matches('/');
        let dir_entry_name = if target.ends_with('/') {
            target.to_string()
        } else {
            format!("{}/", target)
        };

        {
            let mut zip_out = zip::ZipWriter::new(&mut tmp);

            // Copy all existing entries, skipping a duplicate if it already exists
            for i in 0..zip_in.len() {
                let raw_entry = zip_in
                    .by_index_raw(i)
                    .map_err(|e| ArchiveError::Other(format!("read raw entry {i}: {e}")))?;
                let name = raw_entry.name().replace('\\', "/");
                if name == dir_entry_name {
                    // Directory already exists, just copy it and we're done
                    zip_out
                        .raw_copy_file(raw_entry)
                        .map_err(|e| ArchiveError::Other(format!("copy entry {name}: {e}")))?;
                    continue;
                }
                zip_out
                    .raw_copy_file(raw_entry)
                    .map_err(|e| ArchiveError::Other(format!("copy entry {name}: {e}")))?;
            }

            // Add the new directory entry
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip_out
                .add_directory(&dir_entry_name, options)
                .map_err(|e| ArchiveError::Other(format!("add directory: {e}")))?;

            zip_out
                .finish()
                .map_err(|e| ArchiveError::Other(format!("finalize zip: {e}")))?;
        }

        tmp.persist(archive_path)
            .map_err(|e| ArchiveError::Other(format!("replace original archive: {e}")))?;

        Ok(())
    }

    fn write_entry(
        &self,
        archive_path: &Path,
        source_file: &Path,
        inner_path: &str,
        _password: Option<&str>,
        mut progress_cb: Option<&mut dyn FnMut(u64)>,
    ) -> Result<(), ArchiveError> {
        let src_file = std::fs::File::open(archive_path)
            .map_err(|e| ArchiveError::Other(format!("open archive: {e}")))?;
        let mut zip_in = zip::ZipArchive::new(src_file)
            .map_err(|e| ArchiveError::Other(format!("read zip: {e}")))?;

        let parent_dir = archive_path.parent().unwrap_or_else(|| Path::new("."));
        let mut tmp = tempfile::Builder::new()
            .prefix(".flux-update-")
            .suffix(".tmp")
            .tempfile_in(parent_dir)
            .map_err(|e| ArchiveError::Other(format!("create tempfile: {e}")))?;

        let target = inner_path.trim_start_matches('/');
        // For directories we must also evict all child entries so we get a clean write
        let target_dir_prefix = if target.is_empty() {
            String::new()
        } else {
            format!("{}/", target.trim_end_matches('/'))
        };

        {
            let mut zip_out = zip::ZipWriter::new(&mut tmp);

            // Copy entries that do not collide with the target path (file or dir)
            for i in 0..zip_in.len() {
                let raw_entry = zip_in
                    .by_index_raw(i)
                    .map_err(|e| ArchiveError::Other(format!("read raw entry {i}: {e}")))?;
                let name = raw_entry.name().replace('\\', "/");

                if name == target
                    || name == target_dir_prefix
                    || (!target_dir_prefix.is_empty() && name.starts_with(&target_dir_prefix))
                {
                    continue;
                }

                zip_out
                    .raw_copy_file(raw_entry)
                    .map_err(|e| ArchiveError::Other(format!("copy entry {name}: {e}")))?;
            }

            // Write the source (file or directory tree) at inner_path
            fn write_recursive(
                writer: &mut zip::ZipWriter<&mut tempfile::NamedTempFile>,
                src: &Path,
                base_inner: &str,
                progress_cb: &mut Option<&mut dyn FnMut(u64)>,
            ) -> Result<(), ArchiveError> {
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);

                if src.is_dir() {
                    let dir_name = if base_inner.is_empty() {
                        String::new()
                    } else if base_inner.ends_with('/') {
                        base_inner.to_string()
                    } else {
                        format!("{}/", base_inner)
                    };

                    if !dir_name.is_empty() {
                        writer
                            .add_directory(&dir_name, options)
                            .map_err(|e| ArchiveError::Other(format!("add directory: {e}")))?;
                    }

                    for entry in std::fs::read_dir(src)
                        .map_err(|e| ArchiveError::Other(format!("read dir: {e}")))?
                        .flatten()
                    {
                        let child_path = entry.path();
                        let file_name = entry.file_name();
                        let name_str = file_name.to_string_lossy();

                        let child_inner = if dir_name.is_empty() {
                            name_str.to_string()
                        } else {
                            format!("{}{}", dir_name, name_str)
                        };

                        write_recursive(writer, &child_path, &child_inner, progress_cb)?;
                    }
                } else {
                    writer
                        .start_file(base_inner, options)
                        .map_err(|e| ArchiveError::Other(format!("start new entry: {e}")))?;

                    let mut input_data = std::fs::File::open(src)
                        .map_err(|e| ArchiveError::Other(format!("open source file: {e}")))?;

                    let mut buf = [0u8; 65536];
                    loop {
                        let n = input_data
                            .read(&mut buf)
                            .map_err(|e| ArchiveError::Other(format!("read source file: {e}")))?;
                        if n == 0 {
                            break;
                        }
                        writer
                            .write_all(&buf[..n])
                            .map_err(|e| ArchiveError::Other(format!("write entry chunk: {e}")))?;

                        if let Some(ref mut cb) = progress_cb {
                            cb(n as u64);
                        }
                    }
                }
                Ok(())
            }

            write_recursive(&mut zip_out, source_file, target, &mut progress_cb)?;

            zip_out
                .finish()
                .map_err(|e| ArchiveError::Other(format!("finalize zip: {e}")))?;
        }

        tmp.persist(archive_path)
            .map_err(|e| ArchiveError::Other(format!("replace original archive: {e}")))?;

        Ok(())
    }
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

fn extract_dir_zip(
    archive_path: &Path,
    inner_dir: &str,
    password: Option<&str>,
) -> Result<PathBuf, ArchiveError> {
    extract_dir_zip_at(archive_path, inner_dir, password, None)
}

pub(in crate::services::archive) fn extract_dir_zip_at(
    archive_path: &Path,
    inner_dir: &str,
    password: Option<&str>,
    override_dest: Option<&Path>,
) -> Result<PathBuf, ArchiveError> {
    let (temp_dir, dest_dir) = make_dest_dir_at(inner_dir, override_dest)?;

    let prefix = if inner_dir.is_empty() {
        String::new()
    } else {
        format!("{}/", inner_dir.trim_matches('/'))
    };

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

/// Extracts a last-modified unix timestamp from a ZIP entry's `last_modified` field.
fn zip_mtime<R: Read + Seek>(entry: &zip::read::ZipFile<'_, R>) -> i64 {
    entry
        .last_modified()
        .and_then(|dt| {
            chrono::NaiveDate::from_ymd_opt(dt.year() as i32, dt.month() as u32, dt.day() as u32)
                .and_then(|d| {
                    d.and_hms_opt(dt.hour() as u32, dt.minute() as u32, dt.second() as u32)
                })
                .map(|ndt| ndt.and_utc().timestamp())
        })
        .unwrap_or(0)
}
