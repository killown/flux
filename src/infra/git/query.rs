use super::status::GitFileStatus;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

/// Queries the working-tree and staged diff for a single file asynchronously.
pub async fn query_file_diff(repo_root: &Path, target_file: &Path) -> Result<String, String> {
    crate::hit!("query_file_diff");
    let rel_target = target_file.strip_prefix(repo_root).unwrap_or(target_file);

    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["--no-optional-locks", "diff", "HEAD", "--"])
        .arg(rel_target)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err("git diff command failed".to_string());
    }

    let diff_text = String::from_utf8_lossy(&output.stdout).into_owned();
    if diff_text.is_empty() {
        // If file is untracked, show initial status.
        let untracked = Command::new("git")
            .current_dir(repo_root)
            .args(["status", "--porcelain=v1", "--"])
            .arg(rel_target)
            .output()
            .await
            .map_err(|e| e.to_string())?;

        let status_str = String::from_utf8_lossy(&untracked.stdout);
        if status_str.starts_with("??") {
            return Ok(format!("--- Untracked File ---\n{}", target_file.display()));
        }
    }

    Ok(diff_text)
}

/// Runs a non-locking, non-renaming git status command with an asynchronous timeout.
pub async fn query_git_status(
    repo_root: &Path,
    target_dir: &Path,
) -> Result<HashMap<PathBuf, GitFileStatus>, String> {
    crate::hit!("query_git_status");
    let rel_target = target_dir.strip_prefix(repo_root).unwrap_or(Path::new(""));

    let mut cmd = Command::new("git");
    cmd.current_dir(repo_root)
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "--untracked-files=normal",
            "--no-renames",
            "-z",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    if !rel_target.as_os_str().is_empty() {
        cmd.arg("--").arg(rel_target);
    }

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().ok_or("Failed to capture stdout")?;

    let read_result = timeout(Duration::from_millis(1500), async {
        let mut buffer = Vec::new();
        stdout.read_to_end(&mut buffer).await.map(|_| buffer)
    })
    .await;

    let buffer = match read_result {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(e)) => return Err(e.to_string()),
        Err(_) => {
            let _ = child.kill().await;
            return Err("Git status scan timed out".to_string());
        }
    };

    let _ = child.wait().await;

    let mut results = HashMap::new();
    let mut i = 0;
    while i + 3 < buffer.len() {
        let x = buffer[i];
        let y = buffer[i + 1];
        if buffer[i + 2] != b' ' {
            break;
        }

        let start = i + 3;
        let mut end = start;
        while end < buffer.len() && buffer[end] != 0 {
            end += 1;
        }

        if let Ok(rel_path_str) = std::str::from_utf8(&buffer[start..end]) {
            let full_path = repo_root.join(rel_path_str);
            let status = GitFileStatus::from_porcelain(x, y);
            results.insert(full_path, status);
        }

        i = end + 1;
    }

    Ok(results)
}

/// Which side of the index [`query_git_status_paths`] keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    /// Entries with a change recorded in the index.
    Staged,
    /// Entries with a worktree change, untracked files included.
    Unstaged,
}

/// Lists the repository paths that have a `filter` change, as absolute paths.
///
/// A path with both staged and unstaged changes is part of both sets.
pub async fn query_git_status_paths(
    repo_root: &Path,
    filter: StatusFilter,
) -> Result<HashSet<PathBuf>, String> {
    crate::hit!("query_git_status_paths");
    let output = timeout(
        Duration::from_millis(3000),
        Command::new("git")
            .current_dir(repo_root)
            .args([
                "--no-optional-locks",
                "status",
                "--porcelain=v1",
                "--untracked-files=normal",
                "--no-renames",
                "-z",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "Git status scan timed out".to_string())?
    .map_err(|e| e.to_string())?;

    let mut paths = HashSet::new();
    for entry in output.stdout.split(|b| *b == 0) {
        if entry.len() < 4 || entry[2] != b' ' {
            continue;
        }
        let (x, y) = (entry[0], entry[1]);
        let keep = match filter {
            StatusFilter::Staged => x != b' ' && x != b'?' && x != b'!',
            StatusFilter::Unstaged => x != b'!' && y != b' ',
        };
        if !keep {
            continue;
        }
        if let Ok(rel) = std::str::from_utf8(&entry[3..]) {
            paths.insert(repo_root.join(rel));
        }
    }
    Ok(paths)
}
