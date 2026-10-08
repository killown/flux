use super::status::GitFileStatus;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

/// Queries the working-tree and staged diff for a single file asynchronously.
pub async fn query_file_diff(repo_root: &Path, target_file: &Path) -> Result<String, String> {
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
