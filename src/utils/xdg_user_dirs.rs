//! XDG user directories, read from `~/.config/user-dirs.dirs`.
//!
//! That file is written by `xdg-user-dirs-update` and may hold custom entries
//! (for example `XDG_PROJECTS_DIR`) that the `dirs` crate does not expose.

use std::path::{Path, PathBuf};

/// One user directory: its key without the `XDG_` prefix and `_DIR` suffix
/// (for example `PROJECTS`) and its resolved path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDir {
    pub key: String,
    pub path: PathBuf,
}

/// Parses `XDG_<KEY>_DIR="<value>"` lines. `$HOME/...` and `/...` values are
/// resolved; comments and anything else are skipped. Paths are not checked on disk.
pub fn parse_user_dirs(text: &str, home: &Path) -> Vec<UserDir> {
    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("XDG_")?;
            let (key, value) = rest.split_once('=')?;
            let key = key.strip_suffix("_DIR")?;
            if key.is_empty() {
                return None;
            }
            let value = value.strip_prefix('"')?.strip_suffix('"')?;
            let path = if value == "$HOME" {
                home.to_path_buf()
            } else if let Some(rel) = value.strip_prefix("$HOME/") {
                home.join(rel)
            } else if value.starts_with('/') {
                PathBuf::from(value)
            } else {
                return None;
            };
            Some(UserDir {
                key: key.to_string(),
                path,
            })
        })
        .collect()
}

/// The user directories for this session: the entries in `user-dirs.dirs` when
/// that file exists, otherwise the standard ones the `dirs` crate knows about.
pub fn user_dirs(home: &Path) -> Vec<UserDir> {
    let parsed = dirs::config_dir()
        .and_then(|c| std::fs::read_to_string(c.join("user-dirs.dirs")).ok())
        .map(|text| parse_user_dirs(&text, home))
        .unwrap_or_default();
    if !parsed.is_empty() {
        return parsed;
    }

    [
        ("DESKTOP", dirs::desktop_dir()),
        ("DOWNLOAD", dirs::download_dir()),
        ("DOCUMENTS", dirs::document_dir()),
        ("MUSIC", dirs::audio_dir()),
        ("PICTURES", dirs::picture_dir()),
        ("VIDEOS", dirs::video_dir()),
    ]
    .into_iter()
    .filter_map(|(key, path)| {
        path.map(|path| UserDir {
            key: key.to_string(),
            path,
        })
    })
    .collect()
}
