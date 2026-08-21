use std::path::{Path, PathBuf};

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
    /// The destination that already exists.
    pub dest: PathBuf,
    /// `true` when this is a move (cut) rather than a copy.
    pub is_cut: bool,
    /// Total number of files in the current batch (for the dialog subtitle).
    pub batch_total: usize,
    /// 1-based index of the current file within the batch.
    pub batch_index: usize,
}

/// Derives an auto-renamed destination path that does not yet exist.
///
/// Given `/dest/file.txt` returns `/dest/file (1).txt`, `/dest/file (2).txt`, …
/// until a free slot is found.  Works for both files and directories.
pub fn auto_rename_dest(dest: &Path) -> PathBuf {
    let stem = dest
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ext = dest
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = dest.parent().unwrap_or(std::path::Path::new("/"));

    let mut n = 1u32;
    loop {
        let candidate = parent.join(format!("{} ({}){}", stem, n, ext));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}
