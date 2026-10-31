//! Git working-tree status of a single path.

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
}
