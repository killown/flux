//! ZIP backend.
//!
//! Zips may carry a Flux-owned `.flux-cache/` folder, see
//! [`crate::infra::archive::thumb_cache`] for the on-disk layout.
//! A `.flux-cache/` without the sentinel is treated as foreign and left alone.

use crate::infra::archive::entry::collect_entry;
use crate::infra::archive::util::{
    copy_limited, create_no_follow, extract_bytes_via_tempfile, make_dest_dir_at, safe_join,
    MAX_ENTRY_BYTES, MAX_EXTRACT_BYTES,
};
use crate::infra::archive::{
    register_temp_dir, zip_is_encrypted, ArchiveBackend, ArchiveEntry, ArchiveError,
};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use zip::unstable::write::FileOptionsExt;
use zip::HasZipMetadata;

/// Reserved top-level folder that holds Flux-owned data inside a zip.
pub(in crate::infra::archive) const FLUX_CACHE_DIR: &str = ".flux-cache";
/// Sentinel entry proving `.flux-cache/` was created by Flux.
pub(in crate::infra::archive) const FLUX_CACHE_SENTINEL: &str = ".flux-cache/.flux";
/// Metadata entry describing the cache (format version, generator, timestamp).
pub(in crate::infra::archive) const FLUX_CACHE_METADATA: &str = ".flux-cache/metadata.json";
/// Folder holding one cached PNG per thumbnail.
pub(in crate::infra::archive) const FLUX_CACHE_THUMBS_DIR: &str = ".flux-cache/thumbnails";

/// Contents of the sentinel entry.
const FLUX_CACHE_SENTINEL_BODY: &[u8] = b"flux-magic v1";

/// Ensures the "encrypted archive, thumbnail write skipped" notice is logged once.
static ENCRYPTED_WRITE_LOGGED: AtomicBool = AtomicBool::new(false);

/// Returns true for `.flux-cache` itself and anything below it.
#[inline]
fn is_internal_path(name: &str) -> bool {
    name.strip_prefix(FLUX_CACHE_DIR)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Returns the lowercase-hex md5 used as the cached thumbnail's file stem.
fn thumbnail_key(inner_path: &str) -> String {
    format!("{:x}", md5::compute(inner_path.as_bytes()))
}

/// Returns the full in-zip entry name of the cached thumbnail for `inner_path`.
fn thumbnail_entry_name(inner_path: &str) -> String {
    crate::hit!("thumbnail_entry_name");
    format!(
        "{}/{}.png",
        FLUX_CACHE_THUMBS_DIR,
        thumbnail_key(inner_path.trim_start_matches('/'))
    )
}

/// How the entries of an encrypted ZIP are protected, read from the archive
/// itself so that rewrites and new entries keep the same scheme.
#[derive(Clone, Copy)]
enum ZipCipher {
    /// Legacy PKWARE encryption, written through `with_deprecated_encryption`.
    ZipCrypto,
    /// WinZip AES at the key strength the archive already uses.
    Aes(zip::AesMode),
}

/// What a rewrite of an encrypted ZIP needs to keep it encrypted.
#[derive(Clone, Copy)]
struct EncryptionPlan<'p> {
    cipher: ZipCipher,
    password: &'p str,
}

impl<'p> EncryptionPlan<'p> {
    /// Returns `base` with this plan's cipher and password set, for a new entry.
    fn apply(
        &self,
        base: zip::write::SimpleFileOptions,
    ) -> Result<zip::write::FileOptions<'p, ()>, ArchiveError> {
        match self.cipher {
            ZipCipher::ZipCrypto => base
                .with_deprecated_encryption(self.password.as_bytes())
                .map_err(|e| password_error(e, "set up encryption")),
            ZipCipher::Aes(mode) => Ok(base.with_aes_encryption(mode, self.password)),
        }
    }
}

/// Maps a `zip` password failure to the error the UI keys its prompt on.
fn password_error(e: zip::result::ZipError, what: &str) -> ArchiveError {
    match e {
        zip::result::ZipError::InvalidPassword | zip::result::ZipError::UnsupportedArchive(_) => {
            ArchiveError::WrongPassword
        }
        e => ArchiveError::Other(format!("{what}: {e}")),
    }
}

/// Decides whether a rewrite of `zip_in` has to keep it encrypted.
fn encryption_plan<'p, R: Read + Seek>(
    archive_path: &Path,
    zip_in: &mut zip::ZipArchive<R>,
    password: Option<&'p str>,
) -> Result<Option<EncryptionPlan<'p>>, ArchiveError> {
    let mut found: Option<(usize, ZipCipher)> = None;
    for i in 0..zip_in.len() {
        let Ok(raw) = zip_in.by_index_raw(i) else {
            continue;
        };
        if raw.is_file() && raw.encrypted() {
            let cipher = match raw.get_metadata().aes_mode {
                Some((mode, _, _)) => ZipCipher::Aes(mode),
                None => ZipCipher::ZipCrypto,
            };
            found = Some((i, cipher));
            break;
        }
    }

    if found.is_none() && !zip_is_encrypted(archive_path) {
        return Ok(None);
    }
    let Some(password) = password else {
        return Err(ArchiveError::PasswordRequired);
    };
    let Some((idx, cipher)) = found else {
        return Ok(None);
    };

    zip_in
        .by_index_decrypt(idx, password.as_bytes())
        .map(|_| ())
        .map_err(|e| password_error(e, "check password"))?;
    Ok(Some(EncryptionPlan { cipher, password }))
}

/// Returns the name of entry `i`, with `\` normalised to `/`.
fn entry_name<R: Read + Seek>(
    zip_in: &zip::ZipArchive<R>,
    i: usize,
) -> Result<String, ArchiveError> {
    zip_in
        .name_for_index(i)
        .map(|n| n.replace('\\', "/"))
        .ok_or_else(|| ArchiveError::Other(format!("read raw entry {i}: entry not found")))
}

/// Copies entry `i` of `zip_in` into `zip_out` and returns its compressed size.
fn copy_entry<W: Write + Seek>(
    zip_in: &mut zip::ZipArchive<std::fs::File>,
    i: usize,
    zip_out: &mut zip::ZipWriter<W>,
    plan: Option<&EncryptionPlan<'_>>,
) -> Result<u64, ArchiveError> {
    let raw = zip_in
        .by_index_raw(i)
        .map_err(|e| ArchiveError::Other(format!("read raw entry {i}: {e}")))?;
    let name = raw.name().to_owned();
    let size = raw.compressed_size();

    let plan = match plan {
        Some(plan) if raw.is_file() && raw.encrypted() && raw.get_metadata().aes_mode.is_none() => {
            plan
        }
        _ => {
            zip_out
                .raw_copy_file(raw)
                .map_err(|e| ArchiveError::Other(format!("copy entry {name}: {e}")))?;
            return Ok(size);
        }
    };
    drop(raw);

    let mut entry = zip_in
        .by_index_decrypt(i, plan.password.as_bytes())
        .map_err(|e| password_error(e, &format!("decrypt entry {name}")))?;
    let mut options = plan.apply(entry.options())?.into_full_options();
    if !entry.comment().is_empty() {
        options = options.with_file_comment(entry.comment());
    }
    zip_out
        .start_file(name.as_str(), options)
        .map_err(|e| ArchiveError::Other(format!("copy entry {name}: {e}")))?;
    std::io::copy(&mut entry, zip_out)
        .map_err(|e| ArchiveError::Other(format!("re-encrypt entry {name}: {e}")))?;
    Ok(size)
}

/// Gives the rewritten archive the permissions and mtime of the original, so
/// replacing it is invisible apart from the content.
fn restore_attributes(tmp: &tempfile::NamedTempFile, original: &std::fs::Metadata) {
    let _ = std::fs::set_permissions(tmp.path(), original.permissions());
    if let Ok(mtime) = original.modified() {
        let _ = tmp.as_file().set_modified(mtime);
    }
}

pub(in crate::infra::archive) struct ZipBackend;

impl ArchiveBackend for ZipBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        list_zip(archive_path, prefix, password)
    }

    fn total_bytes(
        &self,
        archive_path: &Path,
        _password: Option<&str>,
    ) -> Result<u64, ArchiveError> {
        total_zip(archive_path)
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
        password: Option<&str>,
        mut progress_cb: Option<&mut dyn FnMut(u64)>,
    ) -> Result<(), ArchiveError> {
        let src_file = std::fs::File::open(archive_path)
            .map_err(|e| ArchiveError::Other(format!("open archive: {e}")))?;
        let src_meta = src_file
            .metadata()
            .map_err(|e| ArchiveError::Other(format!("stat archive: {e}")))?;
        let mut zip_in = zip::ZipArchive::new(src_file)
            .map_err(|e| ArchiveError::Other(format!("read zip: {e}")))?;

        let plan = encryption_plan(archive_path, &mut zip_in, password)?;

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
                let name = entry_name(&zip_in, i)?;

                // Skip the entry being deleted (or any child if deleting a directory)
                if name == target || name.starts_with(&target_dir_prefix) {
                    continue;
                }

                let entry_size = copy_entry(&mut zip_in, i, &mut zip_out, plan.as_ref())?;

                if let Some(ref mut cb) = progress_cb {
                    cb(entry_size);
                }
            }

            zip_out
                .finish()
                .map_err(|e| ArchiveError::Other(format!("finalize zip: {e}")))?;
        }

        restore_attributes(&tmp, &src_meta);
        tmp.persist(archive_path)
            .map_err(|e| ArchiveError::Other(format!("replace original archive: {e}")))?;

        Ok(())
    }

    fn create_directory(
        &self,
        archive_path: &Path,
        inner_path: &str,
        password: Option<&str>,
    ) -> Result<(), ArchiveError> {
        let src_file = std::fs::File::open(archive_path)
            .map_err(|e| ArchiveError::Other(format!("open archive: {e}")))?;
        let src_meta = src_file
            .metadata()
            .map_err(|e| ArchiveError::Other(format!("stat archive: {e}")))?;
        let mut zip_in = zip::ZipArchive::new(src_file)
            .map_err(|e| ArchiveError::Other(format!("read zip: {e}")))?;

        let plan = encryption_plan(archive_path, &mut zip_in, password)?;

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
                let name = entry_name(&zip_in, i)?;
                if name == dir_entry_name {
                    // Directory already exists, just copy it and we're done
                    copy_entry(&mut zip_in, i, &mut zip_out, plan.as_ref())?;
                    continue;
                }
                copy_entry(&mut zip_in, i, &mut zip_out, plan.as_ref())?;
            }

            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            let options = match plan.as_ref() {
                Some(plan) => plan.apply(options)?,
                None => options,
            };
            zip_out
                .add_directory(&dir_entry_name, options)
                .map_err(|e| ArchiveError::Other(format!("add directory: {e}")))?;

            zip_out
                .finish()
                .map_err(|e| ArchiveError::Other(format!("finalize zip: {e}")))?;
        }

        restore_attributes(&tmp, &src_meta);
        tmp.persist(archive_path)
            .map_err(|e| ArchiveError::Other(format!("replace original archive: {e}")))?;

        Ok(())
    }

    fn write_entry(
        &self,
        archive_path: &Path,
        source_file: &Path,
        inner_path: &str,
        password: Option<&str>,
        mut progress_cb: Option<&mut dyn FnMut(u64)>,
    ) -> Result<(), ArchiveError> {
        let src_file = std::fs::File::open(archive_path)
            .map_err(|e| ArchiveError::Other(format!("open archive: {e}")))?;
        let src_meta = src_file
            .metadata()
            .map_err(|e| ArchiveError::Other(format!("stat archive: {e}")))?;
        let mut zip_in = zip::ZipArchive::new(src_file)
            .map_err(|e| ArchiveError::Other(format!("read zip: {e}")))?;

        let plan = encryption_plan(archive_path, &mut zip_in, password)?;

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
                let name = entry_name(&zip_in, i)?;

                if name == target
                    || name == target_dir_prefix
                    || (!target_dir_prefix.is_empty() && name.starts_with(&target_dir_prefix))
                {
                    continue;
                }

                copy_entry(&mut zip_in, i, &mut zip_out, plan.as_ref())?;
            }

            // Write the source (file or directory tree) at inner_path
            fn write_recursive(
                writer: &mut zip::ZipWriter<&mut tempfile::NamedTempFile>,
                src: &Path,
                base_inner: &str,
                plan: Option<&EncryptionPlan<'_>>,
                progress_cb: &mut Option<&mut dyn FnMut(u64)>,
            ) -> Result<(), ArchiveError> {
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                let options = match plan {
                    Some(plan) => plan.apply(options)?,
                    None => options,
                };

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

                        write_recursive(writer, &child_path, &child_inner, plan, progress_cb)?;
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

            write_recursive(
                &mut zip_out,
                source_file,
                target,
                plan.as_ref(),
                &mut progress_cb,
            )?;

            zip_out
                .finish()
                .map_err(|e| ArchiveError::Other(format!("finalize zip: {e}")))?;
        }

        restore_attributes(&tmp, &src_meta);
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
    crate::hit!("list_zip");
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

    // 2. We either have a password or the archive is unencrypted: process entries.
    // `.flux-cache/` is hidden only when Flux created it (sentinel present).
    let has_sentinel = zip.index_for_name(FLUX_CACHE_SENTINEL).is_some();
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

        if has_sentinel && is_internal_path(&raw_name) {
            continue;
        }

        collect_entry(&mut seen, &raw_name, is_dir, size, mtime, prefix, false);
    }

    Ok(seen.into_values().collect())
}

/// Sums uncompressed file sizes from the central directory. Raw header access
/// works for encrypted entries too, so no password is needed.
fn total_zip(archive_path: &Path) -> Result<u64, ArchiveError> {
    crate::hit!("total_zip");
    let file =
        std::fs::File::open(archive_path).map_err(|e| ArchiveError::Other(format!("open: {e}")))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| ArchiveError::Other(format!("ZIP: {e}")))?;
    let has_sentinel = zip.index_for_name(FLUX_CACHE_SENTINEL).is_some();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let entry = zip
            .by_index_raw(i)
            .map_err(|e| ArchiveError::Other(format!("ZIP idx {i}: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        if has_sentinel && is_internal_path(entry.name().trim_end_matches('/')) {
            continue;
        }
        total = total.saturating_add(entry.size());
    }
    Ok(total)
}

fn extract_dir_zip(
    archive_path: &Path,
    inner_dir: &str,
    password: Option<&str>,
) -> Result<PathBuf, ArchiveError> {
    extract_dir_zip_at(archive_path, inner_dir, password, None)
}

pub(in crate::infra::archive) fn extract_dir_zip_at(
    archive_path: &Path,
    inner_dir: &str,
    password: Option<&str>,
    override_dest: Option<&Path>,
) -> Result<PathBuf, ArchiveError> {
    crate::hit!("extract_dir_zip_at");
    let (temp_dir, dest_dir) = make_dest_dir_at(inner_dir, override_dest)?;
    let mut budget = MAX_EXTRACT_BYTES;

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

        if is_internal_path(&name) {
            continue;
        }

        // `strip_prefix` removes the prefix exactly once. Trimming it
        // repeatedly would turn `a/a/x` into `x` when extracting `a`.
        let Some(relative) = name.strip_prefix(prefix.as_str()) else {
            continue;
        };
        if relative.is_empty() {
            continue;
        }

        let Some(out_path) = safe_join(&dest_dir, relative) else {
            continue; // skip malicious entry silently
        };
        if is_dir {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| ArchiveError::Other(format!("create dir failed: {e}")))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ArchiveError::Other(format!("create parent dir failed: {e}")))?;
            }
            let mut outfile = create_no_follow(&out_path)
                .map_err(|e| ArchiveError::Other(format!("create file failed: {e}")))?;
            budget -= copy_limited(&mut reader, &mut outfile, budget)
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
    crate::hit!("extract_zip");
    let file =
        std::fs::File::open(archive_path).map_err(|e| ArchiveError::Other(format!("open: {e}")))?;

    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| ArchiveError::Other(format!("ZIP: {e}")))?;

    let idx = zip
        .index_for_name(inner_path)
        .ok_or_else(|| ArchiveError::Other(format!("not found: {inner_path}")))?;

    // The cache is not extractable through the public API.
    if is_internal_path(inner_path) {
        return Err(ArchiveError::Other(format!("not found: {inner_path}")));
    }

    if let Some(pwd) = password {
        // Password supplied: try decrypting
        match zip.by_index_decrypt(idx, pwd.as_bytes()) {
            Ok(mut entry) => {
                copy_limited(&mut entry, &mut tmp, MAX_ENTRY_BYTES)
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
                copy_limited(&mut entry, &mut tmp, MAX_ENTRY_BYTES)
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

// ─── In-archive thumbnail cache ──────────────────────────────────────────────

/// Reads the raw bytes of one entry, bypassing the `.flux-cache/` guard of [`extract_zip`].
fn read_entry_bytes(archive_path: &Path, name: &str, password: Option<&str>) -> Option<Vec<u8>> {
    crate::hit!("read_entry_bytes");
    let file = std::fs::File::open(archive_path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;
    let idx = zip.index_for_name(name)?;
    let mut buf = Vec::new();
    match password {
        Some(pwd) => {
            let mut entry = zip.by_index_decrypt(idx, pwd.as_bytes()).ok()?;
            entry.read_to_end(&mut buf).ok()?;
        }
        None => {
            let mut entry = zip.by_index(idx).ok()?;
            if entry.encrypted() {
                return None;
            }
            entry.read_to_end(&mut buf).ok()?;
        }
    }
    Some(buf)
}

/// Returns the thumbnail cached inside `archive_path` for `inner_path`, if any.
pub fn read_cached_thumbnail(
    archive_path: &Path,
    inner_path: &str,
    password: Option<&str>,
) -> Option<gtk::gdk::Texture> {
    crate::hit!("read_cached_thumbnail");
    // Only trust a cache that Flux itself created.
    {
        let file = std::fs::File::open(archive_path).ok()?;
        let zip = zip::ZipArchive::new(file).ok()?;
        zip.index_for_name(FLUX_CACHE_SENTINEL)?;
    }
    let bytes = read_entry_bytes(archive_path, &thumbnail_entry_name(inner_path), password)?;
    if bytes.is_empty() {
        return None;
    }
    gtk::gdk::Texture::from_bytes(&gtk::glib::Bytes::from_owned(bytes)).ok()
}

/// Rewrites the archive to add `thumbs` under `.flux-cache/`, preserving
/// existing entries' compression. Returns the number written. Skipped (returns
/// `Ok(0)`) for encrypted archives and for a foreign `.flux-cache/`.
pub fn write_thumbnails_to_archive(
    archive_path: &Path,
    thumbs: &[(String, Vec<u8>)],
    password: Option<&str>,
) -> Result<usize, ArchiveError> {
    crate::hit!("write_thumbnails_to_archive");
    let src_file = std::fs::File::open(archive_path)
        .map_err(|e| ArchiveError::Other(format!("open archive: {e}")))?;
    let src_meta = src_file
        .metadata()
        .map_err(|e| ArchiveError::Other(format!("stat archive: {e}")))?;
    let mut zip_in = zip::ZipArchive::new(src_file)
        .map_err(|e| ArchiveError::Other(format!("read zip: {e}")))?;

    // Last write wins when the same inner path was queued twice.
    let mut wanted: Vec<(String, &[u8])> = Vec::with_capacity(thumbs.len());
    for (inner, png) in thumbs {
        let name = thumbnail_entry_name(inner);
        match wanted.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = png.as_slice(),
            None => wanted.push((name, png.as_slice())),
        }
    }
    if wanted.is_empty() {
        return Ok(0);
    }

    let has_sentinel = zip_in.index_for_name(FLUX_CACHE_SENTINEL).is_some();
    if !has_sentinel && zip_in.index_for_name(FLUX_CACHE_METADATA).is_some() {
        return Ok(0);
    }

    let mut any_encrypted = password.is_some();
    for i in 0..zip_in.len() {
        if let Ok(raw) = zip_in.by_index_raw(i) {
            if raw.encrypted() {
                any_encrypted = true;
                break;
            }
        }
    }
    if any_encrypted {
        if !ENCRYPTED_WRITE_LOGGED.swap(true, Ordering::Relaxed) {
            eprintln!("[thumb-cache] encrypted zip: in-archive thumbnails are read-only");
        }
        return Ok(0);
    }

    let parent_dir = archive_path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::Builder::new()
        .prefix(".flux-thumbs-")
        .suffix(".tmp")
        .tempfile_in(parent_dir)
        .map_err(|e| ArchiveError::Other(format!("create tempfile: {e}")))?;

    let replaced: HashSet<&str> = wanted.iter().map(|(n, _)| n.as_str()).collect();
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    {
        let mut zip_out = zip::ZipWriter::new(&mut tmp);

        for i in 0..zip_in.len() {
            let raw_entry = zip_in
                .by_index_raw(i)
                .map_err(|e| ArchiveError::Other(format!("read raw entry {i}: {e}")))?;
            let name = raw_entry.name().replace('\\', "/");

            // Rebuilt below / superseded.
            if name == FLUX_CACHE_METADATA
                || name == FLUX_CACHE_SENTINEL
                || replaced.contains(name.as_str())
            {
                continue;
            }

            zip_out
                .raw_copy_file(raw_entry)
                .map_err(|e| ArchiveError::Other(format!("copy entry {name}: {e}")))?;
        }

        for (name, png) in &wanted {
            zip_out
                .start_file(name.as_str(), options)
                .map_err(|e| ArchiveError::Other(format!("start thumbnail {name}: {e}")))?;
            zip_out
                .write_all(png)
                .map_err(|e| ArchiveError::Other(format!("write thumbnail {name}: {e}")))?;
        }

        let generated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let metadata = serde_json::to_vec(&serde_json::json!({
            "version": 1,
            "generated_by": env!("CARGO_PKG_VERSION"),
            "generated_at": generated_at,
        }))
        .map_err(|e| ArchiveError::Other(format!("encode metadata: {e}")))?;
        zip_out
            .start_file(FLUX_CACHE_METADATA, options)
            .map_err(|e| ArchiveError::Other(format!("start metadata: {e}")))?;
        zip_out
            .write_all(&metadata)
            .map_err(|e| ArchiveError::Other(format!("write metadata: {e}")))?;

        zip_out
            .start_file(FLUX_CACHE_SENTINEL, options)
            .map_err(|e| ArchiveError::Other(format!("start sentinel: {e}")))?;
        zip_out
            .write_all(FLUX_CACHE_SENTINEL_BODY)
            .map_err(|e| ArchiveError::Other(format!("write sentinel: {e}")))?;

        zip_out
            .finish()
            .map_err(|e| ArchiveError::Other(format!("finalize zip: {e}")))?;
    }

    let now_meta = std::fs::metadata(archive_path)
        .map_err(|e| ArchiveError::Other(format!("re-stat archive: {e}")))?;
    if now_meta.len() != src_meta.len() || now_meta.modified().ok() != src_meta.modified().ok() {
        return Err(ArchiveError::Other(
            "archive changed while writing thumbnails".into(),
        ));
    }

    let _ = std::fs::set_permissions(tmp.path(), src_meta.permissions());
    if let Ok(mtime) = src_meta.modified() {
        let _ = tmp.as_file().set_modified(mtime);
    }

    tmp.persist(archive_path)
        .map_err(|e| ArchiveError::Other(format!("replace original archive: {e}")))?;

    Ok(wanted.len())
}
