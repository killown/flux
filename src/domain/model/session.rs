//! Typed groups of `FluxApp` session state.

use crate::domain::model::FilterMode;
use std::collections::HashMap;
use std::path::PathBuf;

/// Git status of the current directory and the file shown in the diff panel.
#[derive(Debug)]
pub struct GitState {
    /// Scanned directory for `status_map`.
    pub status_dir: PathBuf,
    /// Git file status lookup table for entries in `status_dir`.
    pub status_map: std::collections::HashMap<PathBuf, crate::domain::model::GitFileStatus>,
    /// Tracks if active directory belongs to a git repository.
    pub in_repo: bool,
    /// File path currently displayed inside the git diff review panel.
    pub active_diff_target: Option<PathBuf>,
}

/// Session-scoped filters applied to every directory load.
#[derive(Debug)]
pub struct Filters {
    /// Ephemeral MIME filter: only entries whose MIME type equals this are shown.
    pub mime: Option<String>,
    /// True when the current `mime` was applied by the `Space` shortcut,
    /// meaning closing the preview panel should also clear it.
    pub mime_from_space: bool,
    /// Session-scoped glob pattern allowlist for persistent navigation filtering.
    pub extension: Option<Vec<String>>,
    /// Precompiled glob matcher for the current extension filter.
    pub extension_globset: Option<globset::GlobSet>,
    /// Session-scoped folder/file filter applied to every directory load, persists across navigation.
    pub mode: FilterMode,
}

/// Password state for encrypted archives browsed this session.
#[derive(Debug)]
pub struct ArchiveState {
    /// True while the current archive path requires a password that has not yet been supplied.
    pub locked: bool,
    /// Passwords resolved this session, keyed by the archive's on-disk path.
    pub passwords: HashMap<PathBuf, String>,
    /// Archives this session has seen demand a password (7z and RAR cannot be probed
    /// cheaply). Paths only, in memory only, feeds the padlock badge.
    pub known_encrypted: std::collections::HashSet<PathBuf>,
    /// Keyring saves the user requested, held back until the archive accepts the password.
    pub pending_keyring_saves: crate::domain::model::PendingSaves,
}

/// The pinned directory list and the named Quick Lists behind it.
#[derive(Debug)]
pub struct QuickState {
    /// A pinned list of directories for quick multi-context switching.
    pub list: Vec<PathBuf>,
    /// The index of the currently active item in the exclusive directory list.
    pub index: Option<usize>,
    /// Every named Quick List. `list` and `index` always
    /// mirror the selected one, so this copy of the selected list goes stale.
    /// Read it through `FluxApp::quick_snapshot`.
    pub groups: crate::domain::model::quick_groups::QuickGroups,
}
