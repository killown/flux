use super::query::query_git_status;
use super::status::GitFileStatus;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Combines recursive file statuses for directory summaries.
fn rollup_status(current: Option<GitFileStatus>, new: GitFileStatus) -> GitFileStatus {
    match (current, new) {
        (Some(GitFileStatus::Conflicted), _) | (_, GitFileStatus::Conflicted) => {
            GitFileStatus::Conflicted
        }
        (None, GitFileStatus::Untracked)
        | (Some(GitFileStatus::Untracked), GitFileStatus::Untracked) => GitFileStatus::Untracked,
        _ => GitFileStatus::Modified,
    }
}

/// Adds a rolled-up status for each direct child of `target_dir` holding
/// changes. Exact entries win over rollups.
pub fn with_directory_rollup(
    target_dir: &Path,
    mut statuses: HashMap<PathBuf, GitFileStatus>,
) -> HashMap<PathBuf, GitFileStatus> {
    let mut rollup: HashMap<PathBuf, GitFileStatus> = HashMap::new();

    for (path, status) in &statuses {
        let Ok(rest) = path.strip_prefix(target_dir) else {
            continue;
        };
        let mut components = rest.components();
        let Some(first) = components.next() else {
            continue;
        };
        if components.next().is_none() {
            continue;
        }
        let child = target_dir.join(first.as_os_str());
        let merged = rollup_status(rollup.get(&child).copied(), *status);
        rollup.insert(child, merged);
    }

    for (child, status) in rollup {
        statuses.entry(child).or_insert(status);
    }
    statuses
}

/// Scans git status for `target_dir` and returns exact entries plus
/// per-child directory rollups.
pub async fn query_git_status_for_view(
    repo_root: &Path,
    target_dir: &Path,
) -> Result<HashMap<PathBuf, GitFileStatus>, String> {
    let raw = query_git_status(repo_root, target_dir).await?;
    Ok(with_directory_rollup(target_dir, raw))
}
