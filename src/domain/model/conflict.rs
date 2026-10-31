//! Conflict-resolution data for paste and transfer operations.

use std::path::PathBuf;

/// The resolution choice for a single conflicting file or directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictChoice {
    /// Overwrite the destination with the source.
    Replace,
    /// Skip this file, leave the destination untouched and continue the batch.
    Skip,
    /// Write the source to an auto-suffixed name, e.g. `file (1).txt`.
    AutoRename,
    /// Abort the entire batch operation.
    Cancel,
}

/// Session-scoped policy that collapses future conflict prompts for the
/// remainder of one batch job.  Starts as `Ask` and flips when the user
/// enables "Apply to all".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConflictPolicy {
    /// Show the dialog for each conflict (default).
    #[default]
    Ask,
    /// Silently overwrite every subsequent conflicting destination.
    ReplaceAll,
    /// Silently skip every subsequent conflicting destination.
    SkipAll,
    /// Silently rename every subsequent conflicting file.
    AutoRenameAll,
}

/// Context passed to the UI thread so it can build the conflict dialog.
#[derive(Debug, Clone)]
pub struct ConflictContext {
    /// The source path that is being copied or moved.
    pub src: PathBuf,
    /// The destination that already exists.
    pub dest: PathBuf,
    /// `true` when this is a move (cut) rather than a copy.
    pub is_cut: bool,
    /// Total number of files in the current batch (for the dialog subtitle).
    pub batch_total: usize,
    /// 1-based index of the current file within the batch.
    pub batch_index: usize,
}
