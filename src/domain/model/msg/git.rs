//! Git actions, status views and the diff panel.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum GitMsg {
    /// Background result of a git action, `Ok` is a short success label,
    /// `Err` is the human-readable error to surface as a toast.
    GitActionFinished(Result<String, String>),
    /// Reverts only the selected files to HEAD, after backing their diff up to `/tmp`.
    GitDiscardSelected,
    /// Re-applies the backup diff written by the last discard.
    GitRestoreLastDiscard,
    /// Menu entry: lists the local branches, then opens the branch dialog.
    PromptGitSwitchBranch,
    /// Emitted after `PromptGitSwitchBranch`, opens the branch dialog.
    GitBranchesReady {
        current: Option<String>,
        branches: Vec<String>,
    },
    /// Runs `git switch <name>`.
    GitSwitchBranch(String),
    /// Runs `git switch -c <name>`.
    GitCreateBranch(String),
    /// Menu entry: copies `git diff HEAD` to the clipboard.
    GitCopyDiff,
    /// Emitted after `GitCopyDiff` with the diff text, copied on the GTK thread.
    GitCopyDiffReady(String),
    /// Menu entry: shows `git log` for the selection (or the current directory).
    GitShowLog,
    /// Shows read-only git output in a dialog.
    GitOutputReady { title: String, text: String },
    /// Opens the `origin` web page of the current branch, or of the selected file or folder.
    GitOpenRemote,
    /// Loads only the files with staged changes into the view.
    ShowGitStatusStaged,
    /// Loads only the files with unstaged or untracked changes into the view.
    ShowGitStatusUnstaged,
    /// Opens the currently active diff target at the specified line number with an optional forward search pattern.
    OpenActiveDiffLine { line: usize, query: Option<String> },
    /// Toggles the visibility of the right-side git diff review sidebar.
    ToggleDiffPanel,
    /// Requests the git diff for a specific file and reveals the diff panel.
    ShowFileDiff(PathBuf),
    /// Delivers the loaded git diff output for a target file to update the view.
    DiffLoaded { path: PathBuf, diff: String },
    /// Updates and persists the width of the git diff review panel.
    SetDiffPanelWidth(i32),
    /// Toggles whether selecting a modified file automatically opens the diff sidebar.
    SetAutoShowDiff(bool),
    /// Filters grid view to files with uncommitted git changes.
    ShowGitStatusView,
    /// Toggles the header git status button.
    SetGitRepoActive(bool),
    /// Emitted when background git status scan finishes.
    GitStatusReady {
        path: PathBuf,
        load_id: u64,
        updates: std::collections::HashMap<PathBuf, crate::infra::git::GitFileStatus>,
    },
}
