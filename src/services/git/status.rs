use super::font::is_nerd_font_available;

/// Git working tree and index status.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GitFileStatus {
    #[default]
    None,
    Untracked,
    Added,
    Modified,
    StagedModified,
    MixedModified,
    Deleted,
    Renamed,
    TypeChanged,
    Ignored,
    Conflicted,
}

impl GitFileStatus {
    /// Parses porcelain v1 index and worktree status bytes.
    pub fn from_porcelain(x: u8, y: u8) -> Self {
        if x == b'U' || y == b'U' || (x == b'A' && y == b'A') || (x == b'D' && y == b'D') {
            return Self::Conflicted;
        }

        match (x, y) {
            (b'?', b'?') => Self::Untracked,
            (b'!', b'!') => Self::Ignored,
            (b'A', b' ') => Self::Added,
            (b'A', b'M') => Self::MixedModified,
            (b'M', b' ') => Self::StagedModified,
            (b' ', b'M') => Self::Modified,
            (b'M', b'M') => Self::MixedModified,
            (b'D', _) | (_, b'D') => Self::Deleted,
            (b'R', _) => Self::Renamed,
            (b'T', _) | (_, b'T') => Self::TypeChanged,
            _ => Self::Modified,
        }
    }

    /// Returns display badge emblem and CSS class name.
    ///
    /// The emblem is a Nerd Font glyph when one is installed, otherwise a
    /// plain ASCII/Unicode substitute.
    pub fn badge_info(&self) -> Option<(&'static str, &'static str)> {
        if is_nerd_font_available() {
            match self {
                Self::None | Self::Ignored => None,
                Self::Untracked => Some(("\u{e702}", "flux-git-untracked")),
                Self::Added => Some(("\u{ec6d}", "flux-git-added")),
                Self::Modified => Some(("\u{ec6c}", "flux-git-modified")),
                Self::StagedModified => Some(("\u{ec6d}", "flux-git-staged")),
                Self::MixedModified => Some(("\u{ec6c}", "flux-git-mixed")),
                Self::Deleted => Some(("\u{eafc}", "flux-git-deleted")),
                Self::Renamed => Some(("\u{eafd}", "flux-git-renamed")),
                Self::TypeChanged => Some(("\u{e728}", "flux-git-typechange")),
                Self::Conflicted => Some(("\u{ec6e}", "flux-git-conflict")),
            }
        } else {
            match self {
                Self::None | Self::Ignored => None,
                Self::Untracked => Some(("?", "flux-git-untracked")),
                Self::Added => Some(("+", "flux-git-added")),
                Self::Modified => Some(("~", "flux-git-modified")),
                Self::StagedModified => Some(("●", "flux-git-staged")),
                Self::MixedModified => Some(("~●", "flux-git-mixed")),
                Self::Deleted => Some(("-", "flux-git-deleted")),
                Self::Renamed => Some(("»", "flux-git-renamed")),
                Self::TypeChanged => Some(("T", "flux-git-typechange")),
                Self::Conflicted => Some(("!", "flux-git-conflict")),
            }
        }
    }
}
