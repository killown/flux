//! 7-Zip backend.

use crate::services::archive::entry::collect_entry;
use crate::services::archive::util::{extract_bytes_via_tempfile, make_dest_dir_at};
use crate::services::archive::{register_temp_dir, ArchiveBackend, ArchiveEntry, ArchiveError};
use sevenz_rust2::{ArchiveReader, Password};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub(in crate::services::archive) struct SevenZBackend;

impl ArchiveBackend for SevenZBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        list_7z(archive_path, prefix, password)
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

fn list_7z(
    archive_path: &Path,
    prefix: &str,
    password: Option<&str>,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| ArchiveError::Other(format!("open: {e}")))?;

    let pwd: Password = password.map(Password::from).unwrap_or_else(Password::empty);

    let reader = ArchiveReader::new(file, pwd).map_err(|e| match e {
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
                    std::io::copy(r, &mut tmp).map_err(sevenz_rust2::Error::from)?;
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

pub(in crate::services::archive) fn extract_dir_7z_at(
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
                if !raw.starts_with(&prefix) {
                    std::io::copy(r, &mut std::io::sink()).map_err(sevenz_rust2::Error::from)?;
                    return Ok(true);
                }
                let relative = raw.trim_start_matches(prefix.as_str());
                if relative.is_empty() {
                    std::io::copy(r, &mut std::io::sink()).map_err(sevenz_rust2::Error::from)?;
                    return Ok(true);
                }

                let out_path = dest_dir.join(relative);
                if entry.is_directory {
                    std::fs::create_dir_all(&out_path).ok();
                    std::io::copy(r, &mut std::io::sink()).map_err(sevenz_rust2::Error::from)?;
                } else {
                    if let Some(parent) = out_path.parent() {
                        std::fs::create_dir_all(parent).ok();
                    }
                    let mut outfile = std::fs::File::create(&out_path).map_err(|e| {
                        sevenz_rust2::Error::from(std::io::Error::other(e.to_string()))
                    })?;
                    std::io::copy(r, &mut outfile).map_err(sevenz_rust2::Error::from)?;
                }
                Ok(true)
            },
        )
        .map_err(|e| ArchiveError::Other(e.to_string()))?;

    let result = dest_dir.clone();
    register_temp_dir(temp_dir);
    Ok(result)
}
