use super::font::is_nerd_font_available;

pub use crate::domain::model::GitFileStatus;

impl GitFileStatus {
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
