//! RAR backend, shells out to `unar` or `unrar`.

use crate::services::archive::entry::collect_entry;
use crate::services::archive::util::{
    extract_bytes_via_tempfile, find_file_recursive, make_dest_dir,
};
use crate::services::archive::{register_temp_dir, ArchiveBackend, ArchiveEntry, ArchiveError};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(in crate::services::archive) struct RarBackend;

impl ArchiveBackend for RarBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        list_rar(archive_path, prefix, password)
    }

    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        inner_path: &str,
        password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        extract_bytes_via_tempfile(archive_path, inner_path, password, |p, i, pw, tmp| {
            extract_rar(p, i, pw, tmp)
        })
    }

    fn extract_dir(
        &self,
        archive_path: &Path,
        inner_dir: &str,
        password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        let (temp_dir, dest_dir) = make_dest_dir(inner_dir)?;

        let tool = rar_tool().ok_or_else(|| {
            ArchiveError::Other(
                "RAR support requires 'unar' or 'unrar' (install via your package manager)"
                    .to_owned(),
            )
        })?;

        let mut cmd = std::process::Command::new(tool);
        match tool {
            "unar" => {
                cmd.arg("-no-directory")
                    .arg("-output-directory")
                    .arg(&dest_dir);
                if let Some(pwd) = password {
                    cmd.arg("-password").arg(pwd);
                }
                cmd.arg(archive_path);
                if !inner_dir.is_empty() {
                    cmd.arg(inner_dir);
                }
            }
            _ => {
                cmd.arg("x").arg("-y");
                if let Some(pwd) = password {
                    cmd.arg(format!("-p{pwd}"));
                } else {
                    cmd.arg("-p-");
                }
                cmd.arg(archive_path);
                if !inner_dir.is_empty() {
                    cmd.arg(inner_dir);
                }
                cmd.arg(format!("{}/", dest_dir.display()));
            }
        }

        let output = cmd
            .output()
            .map_err(|e| ArchiveError::Other(format!("{tool} spawn: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("password") || stderr.contains("encrypted") {
                return Err(if password.is_none() {
                    ArchiveError::PasswordRequired
                } else {
                    ArchiveError::WrongPassword
                });
            }
            return Err(ArchiveError::Other(format!(
                "{tool} extract failed: {}",
                stderr.trim()
            )));
        }

        let result = dest_dir.clone();
        register_temp_dir(temp_dir);
        Ok(result)
    }
}

// ─── RAR ─────────────────────────────────────────────────────────────────────
//
// No pure-Rust RAR crate exists (RAR is proprietary). We shell out to `unar`
// (The Unarchiver) or `unrar` as a fallback.
// Install: sudo apt install unar   OR   sudo pacman -S unar

/// Returns the first RAR CLI tool found on PATH, or `None` if neither is available.
fn rar_tool() -> Option<&'static str> {
    ["unar", "unrar"]
        .iter()
        .find(|&&tool| {
            std::process::Command::new(tool)
                .arg("--version")
                .output()
                .is_ok()
        })
        .copied()
}

fn list_rar(
    archive_path: &Path,
    prefix: &str,
    password: Option<&str>,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let tool = rar_tool().ok_or_else(|| {
        ArchiveError::Other(crate::i18n::tr(
            "RAR support requires 'unar' or 'unrar' (install via your package manager)",
        ))
    })?;

    let mut cmd = std::process::Command::new(tool);
    match tool {
        "unar" => {
            cmd.arg("-list").arg(archive_path);
            if let Some(pwd) = password {
                cmd.arg("-password").arg(pwd);
            }
        }
        _ => {
            cmd.arg("l");
            cmd.arg(if let Some(pwd) = password {
                format!("-p{pwd}")
            } else {
                "-p-".to_owned()
            });
            cmd.arg(archive_path);
        }
    }

    let output = cmd
        .output()
        .map_err(|e| ArchiveError::Other(format!("{tool} spawn: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("password") || stderr.contains("encrypted") {
            return Err(if password.is_none() {
                ArchiveError::PasswordRequired
            } else {
                ArchiveError::WrongPassword
            });
        }
        return Err(ArchiveError::Other(format!(
            "{tool} failed ({}): {}",
            output.status,
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut seen: HashMap<String, ArchiveEntry> = HashMap::new();

    if tool == "unar" {
        parse_unar_list(&stdout, prefix, &mut seen);
    } else {
        parse_unrar_list(&stdout, prefix, &mut seen);
    }

    Ok(seen.into_values().collect())
}

/// Parses `unar -list` tab-separated output, name is the last tab-delimited field.
fn parse_unar_list(stdout: &str, prefix: &str, seen: &mut HashMap<String, ArchiveEntry>) {
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("Archive")
            || line.starts_with("---")
            || line.starts_with("===")
        {
            continue;
        }
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        let raw_name = parts
            .last()
            .map(|s| s.trim().replace('\\', "/"))
            .unwrap_or_default();
        let raw_name = raw_name.trim_end_matches('/');
        if raw_name.is_empty() {
            continue;
        }
        let size: u64 = parts
            .first()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let is_dir = size == 0 && raw_name.contains('/');
        collect_entry(seen, raw_name, is_dir, size, 0, prefix, false);
    }
}

/// Parses `unrar l` fixed-width output, listing is delimited by `---` separator lines.
fn parse_unrar_list(stdout: &str, prefix: &str, seen: &mut HashMap<String, ArchiveEntry>) {
    let mut in_listing = false;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("---") {
            in_listing = !in_listing;
            continue;
        }
        if !in_listing || trimmed.is_empty() {
            continue;
        }
        let raw_name = match line.split_whitespace().last() {
            Some(n) => n.replace('\\', "/"),
            None => continue,
        };
        let raw_name = raw_name.trim_end_matches('/').to_owned();
        if raw_name.is_empty() {
            continue;
        }
        let is_dir = line
            .chars()
            .next()
            .map(|c| c == 'd' || c == 'D')
            .unwrap_or(false);
        let size: u64 = line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        collect_entry(seen, &raw_name, is_dir, size, 0, prefix, false);
    }
}

fn extract_rar(
    archive_path: &Path,
    inner_path: &str,
    password: Option<&str>,
    mut tmp: tempfile::NamedTempFile,
) -> Result<tempfile::NamedTempFile, ArchiveError> {
    let tool = rar_tool().ok_or_else(|| {
        ArchiveError::Other(
            "RAR support requires 'unar' or 'unrar' (install via your package manager)".to_owned(),
        )
    })?;

    let tmp_dir = tempfile::tempdir().map_err(|e| ArchiveError::Other(format!("tempdir: {e}")))?;

    let mut cmd = std::process::Command::new(tool);
    match tool {
        "unar" => {
            cmd.arg("-output-directory")
                .arg(tmp_dir.path())
                .arg(archive_path)
                .arg(inner_path);
            if let Some(pwd) = password {
                cmd.arg("-password").arg(pwd);
            }
        }
        _ => {
            cmd.arg("e");
            cmd.arg(if let Some(pwd) = password {
                format!("-p{pwd}")
            } else {
                "-p-".to_owned()
            });
            cmd.arg(archive_path).arg(inner_path).arg(tmp_dir.path());
        }
    }

    let output = cmd
        .output()
        .map_err(|e| ArchiveError::Other(format!("{tool} spawn: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("password") || stderr.contains("encrypted") {
            return Err(if password.is_none() {
                ArchiveError::PasswordRequired
            } else {
                ArchiveError::WrongPassword
            });
        }
        return Err(ArchiveError::Other(format!(
            "{tool} extract failed: {}",
            stderr.trim()
        )));
    }

    let file_name = std::path::Path::new(inner_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(inner_path);

    let extracted = find_file_recursive(tmp_dir.path(), file_name)?;

    let mut src = std::fs::File::open(&extracted)
        .map_err(|e| ArchiveError::Other(format!("open extracted: {e}")))?;
    std::io::copy(&mut src, &mut tmp).map_err(|e| ArchiveError::Other(format!("copy: {e}")))?;
    tmp.flush()
        .map_err(|e| ArchiveError::Other(format!("flush: {e}")))?;
    Ok(tmp)
}
