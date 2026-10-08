use std::path::{Path, PathBuf};

/// Finds `.git` repository root by traversing parent paths.
///
/// Handles both in-tree `.git` directories and the `.git` file that worktrees
/// and submodules use (which contains a `gitdir:` pointer).
pub fn find_git_repo_root(start: &Path) -> Option<PathBuf> {
    let mut curr = start.to_path_buf();
    loop {
        let dot_git = curr.join(".git");
        if dot_git.is_dir() {
            return Some(curr);
        } else if dot_git.is_file() {
            if let Ok(content) = std::fs::read_to_string(&dot_git) {
                if content.trim().starts_with("gitdir:") {
                    return Some(curr);
                }
            }
        }
        if !curr.pop() {
            break;
        }
    }
    None
}
