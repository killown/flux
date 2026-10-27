//! 7-Zip backend.

use crate::infra::archive::entry::collect_entry;
use crate::infra::archive::util::{
    copy_limited, create_no_follow, extract_bytes_via_tempfile, make_dest_dir_at, safe_join,
    MAX_ENTRY_BYTES, MAX_EXTRACT_BYTES,
};
use crate::infra::archive::{register_temp_dir, ArchiveBackend, ArchiveEntry, ArchiveError};
use sevenz_rust2::{ArchiveReader, Password};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub(in crate::infra::archive) struct SevenZBackend;

impl ArchiveBackend for SevenZBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        list_7z(archive_path, prefix, password)
    }

    fn total_bytes(
        &self,
        archive_path: &Path,
        password: Option<&str>,
    ) -> Result<u64, ArchiveError> {
        total_7z(archive_path, password)
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

// ─── 7-Zip ───────────────────────────────────────────────────────────────────

/// Opens the archive header, mapping password failures to [`ArchiveError`].
fn open_7z_reader(
    archive_path: &Path,
    password: Option<&str>,
) -> Result<ArchiveReader<std::fs::File>, ArchiveError> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| ArchiveError::Other(format!("open: {e}")))?;

    let pwd: Password = password.map(Password::from).unwrap_or_else(Password::empty);

    ArchiveReader::new(file, pwd).map_err(|e| match e {
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
    })
}

/// Sums uncompressed file sizes from the header, without decompressing.
fn total_7z(archive_path: &Path, password: Option<&str>) -> Result<u64, ArchiveError> {
    crate::hit!("total_7z");
    let reader = open_7z_reader(archive_path, password)?;
    Ok(reader
        .archive()
        .files
        .iter()
        .filter(|e| !e.is_directory)
        .fold(0u64, |acc, e| acc.saturating_add(e.size)))
}

fn list_7z(
    archive_path: &Path,
    prefix: &str,
    password: Option<&str>,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    crate::hit!("list_7z");
    let reader = open_7z_reader(archive_path, password)?;

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
    crate::hit!("extract_7z");
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
                    copy_limited(r, &mut tmp, MAX_ENTRY_BYTES)
                        .map_err(sevenz_rust2::Error::from)?;
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
    extract_dir_7z_at(archive_path, inner_dir, password, None)
}

pub(in crate::infra::archive) fn extract_dir_7z_at(
    archive_path: &Path,
    inner_dir: &str,
    password: Option<&str>,
    override_dest: Option<&Path>,
) -> Result<PathBuf, ArchiveError> {
    crate::hit!("extract_dir_7z_at");
    let (temp_dir, dest_dir) = make_dest_dir_at(inner_dir, override_dest)?;
    let mut budget = MAX_EXTRACT_BYTES;

    let prefix = if inner_dir.is_empty() {
        String::new()
    } else {
        format!("{}/", inner_dir.trim_matches('/'))
    };

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
                // `strip_prefix` removes the prefix exactly once. Trimming it
                // repeatedly would turn `a/a/x` into `x` when extracting `a`.
                let Some(relative) = raw.strip_prefix(prefix.as_str()) else {
                    std::io::copy(r, &mut std::io::sink()).map_err(sevenz_rust2::Error::from)?;
                    return Ok(true);
                };
                if relative.is_empty() {
                    std::io::copy(r, &mut std::io::sink()).map_err(sevenz_rust2::Error::from)?;
                    return Ok(true);
                }

                let Some(out_path) = safe_join(&dest_dir, relative) else {
                    std::io::copy(r, &mut std::io::sink()).map_err(sevenz_rust2::Error::from)?;
                    return Ok(true);
                };
                if entry.is_directory {
                    std::fs::create_dir_all(&out_path).ok();
                    std::io::copy(r, &mut std::io::sink()).map_err(sevenz_rust2::Error::from)?;
                } else {
                    if let Some(parent) = out_path.parent() {
                        std::fs::create_dir_all(parent).ok();
                    }
                    let mut outfile = create_no_follow(&out_path).map_err(|e| {
                        sevenz_rust2::Error::from(std::io::Error::other(e.to_string()))
                    })?;
                    budget -=
                        copy_limited(r, &mut outfile, budget).map_err(sevenz_rust2::Error::from)?;
                }
                Ok(true)
            },
        )
        .map_err(|e| ArchiveError::Other(e.to_string()))?;

    let result = dest_dir.clone();
    register_temp_dir(temp_dir);
    Ok(result)
}
