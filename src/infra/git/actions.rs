//! One-shot git commands triggered from the header menu.

use crate::i18n::tr;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;
use tokio::process::Command;

/// Upper bound for local, quick commands.
const LOCAL_TIMEOUT: Duration = Duration::from_secs(30);

/// The backup diff written by the last discard, kept so [`restore_last_discard`] can re-apply it.
struct LastDiscard {
    root: PathBuf,
    backup: PathBuf,
}

static LAST_DISCARD: Mutex<Option<LastDiscard>> = Mutex::new(None);

/// Base `git` command for `repo_root`: no stdin, never prompts on a terminal, killed when dropped.
fn git_cmd(repo_root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_root)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    cmd
}

/// Like [`git_cmd`], but pathspecs are literal so names such as `a[1].txt` are never read as globs.
fn git_cmd_literal(repo_root: &Path) -> Command {
    let mut cmd = git_cmd(repo_root);
    cmd.env("GIT_LITERAL_PATHSPECS", "1");
    cmd
}

/// Joins git's stderr and stdout into trimmed, non-empty lines for the message dialog.
fn failure_text(out: &std::process::Output) -> String {
    let err = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    err.lines()
        .chain(stdout.lines())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Runs `cmd` to completion within `limit`, mapping a non-zero exit to `"<what> failed: <output>"`.
///
/// `what` is a plain command name (`"git diff"`, …), never translated - it is the literal
/// CLI command being run. The surrounding sentence is translated.
async fn run(
    mut cmd: Command,
    what: &str,
    limit: Duration,
) -> Result<std::process::Output, String> {
    let out = tokio::time::timeout(limit, cmd.output())
        .await
        .map_err(|_| {
            tr("{} timed out after {}s")
                .replace("{}", what)
                .replace("{}", &limit.as_secs().to_string())
        })?
        .map_err(|e| format!("{what} spawn: {e}"))?;
    if !out.status.success() {
        let msg = tr("{} failed: {}")
            .replace("{}", what)
            .replace("{}", &failure_text(&out));
        return Err(msg);
    }
    Ok(out)
}

/// Splits `git ... -z` output into paths.
fn split_nul_paths(bytes: &[u8]) -> Vec<PathBuf> {
    bytes
        .split(|b| *b == 0)
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| PathBuf::from(OsStr::from_bytes(chunk)))
        .collect()
}

/// Turns absolute paths into paths relative to `repo_root`, dropping the root itself and anything outside it.
pub fn repo_relative(repo_root: &Path, paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter_map(|p| p.strip_prefix(repo_root).ok().map(Path::to_path_buf))
        .filter(|p| !p.as_os_str().is_empty())
        .collect()
}

/// Writes `diff` to `/tmp/flux-<random>.diff` and returns the path.
fn write_backup_diff(diff: &[u8]) -> Result<PathBuf, String> {
    let mut path = std::env::temp_dir();
    path.push(format!("flux-{}.diff", random_suffix()));
    let mut file = std::fs::File::create(&path).map_err(|e| {
        format!(
            "{}: {e}",
            tr("Could not create file {}").replace("{}", &path.display().to_string())
        )
    })?;
    file.write_all(diff).map_err(|e| {
        format!(
            "{}: {e}",
            tr("Could not write file {}").replace("{}", &path.display().to_string())
        )
    })?;
    Ok(path)
}

fn remember_discard(repo_root: &Path, backup: &Path) {
    *LAST_DISCARD.lock().unwrap_or_else(|e| e.into_inner()) = Some(LastDiscard {
        root: repo_root.to_path_buf(),
        backup: backup.to_path_buf(),
    });
}

/// 12-char lowercase-hex suffix from `getrandom`-ish sources. Falls back to
/// nanos + pid if `/dev/urandom` is unavailable.
fn random_suffix() -> String {
    use std::io::Read;
    let mut buf = [0u8; 6];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .is_ok()
    {
        return buf.iter().map(|b| format!("{:02x}", b)).collect();
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}{:x}", std::process::id(), nanos)
        .chars()
        .take(12)
        .collect()
}

/// Reverts only `paths` (relative to `repo_root`) to HEAD, after backing their diff up to `/tmp`.
///
/// Files newly added to the index are removed outright, everything else is restored from HEAD.
/// Untracked files are never touched.
pub async fn discard_selected(repo_root: &Path, paths: &[PathBuf]) -> Result<String, String> {
    crate::hit!("discard_selected");
    if paths.is_empty() {
        return Err(tr("Nothing selected inside this repository"));
    }

    let mut names = git_cmd_literal(repo_root);
    names
        .args(["diff", "HEAD", "--name-only", "-z", "--no-renames", "--"])
        .args(paths);
    let changed = split_nul_paths(&run(names, "git diff", LOCAL_TIMEOUT).await?.stdout);
    if changed.is_empty() {
        return Ok(tr("Nothing to discard in the selection"));
    }

    let mut added_cmd = git_cmd_literal(repo_root);
    added_cmd
        .args([
            "diff",
            "HEAD",
            "--name-only",
            "-z",
            "--no-renames",
            "--diff-filter=A",
            "--",
        ])
        .args(paths);
    let added: HashSet<PathBuf> =
        split_nul_paths(&run(added_cmd, "git diff", LOCAL_TIMEOUT).await?.stdout)
            .into_iter()
            .collect();

    let mut diff = git_cmd_literal(repo_root);
    diff.args([
        "diff",
        "HEAD",
        "--binary",
        "--no-color",
        "--no-ext-diff",
        "--no-renames",
        "--",
    ])
    .args(&changed);
    let backup = write_backup_diff(&run(diff, "git diff", LOCAL_TIMEOUT).await?.stdout)?;

    let tracked: Vec<&PathBuf> = changed.iter().filter(|p| !added.contains(*p)).collect();
    if !tracked.is_empty() {
        let mut restore = git_cmd_literal(repo_root);
        restore
            .args(["restore", "--source=HEAD", "--staged", "--worktree", "--"])
            .args(tracked);
        run(restore, "git restore", LOCAL_TIMEOUT).await?;
    }
    if !added.is_empty() {
        let mut remove = git_cmd_literal(repo_root);
        remove.args(["rm", "-f", "-q", "--"]).args(&added);
        run(remove, "git rm", LOCAL_TIMEOUT).await?;
    }

    remember_discard(repo_root, &backup);
    let msg = tr("Discarded {} file(s); backup at {}")
        .replace("{}", &changed.len().to_string())
        .replace("{}", &backup.display().to_string());
    Ok(msg)
}

/// Re-applies the backup written by the last discard (`git apply --index`, then without `--index`).
pub async fn restore_last_discard(repo_root: &Path) -> Result<String, String> {
    crate::hit!("restore_last_discard");
    let last = LAST_DISCARD
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|l| (l.root.clone(), l.backup.clone()));
    let Some((root, backup)) = last else {
        return Err(tr("No discard to restore"));
    };
    if root != repo_root {
        let msg = tr("The last discard happened in another repository: {}")
            .replace("{}", &root.display().to_string());
        return Err(msg);
    }
    if !backup.is_file() {
        let msg = tr("Backup no longer exists: {}").replace("{}", &backup.display().to_string());
        return Err(msg);
    }

    let mut staged = git_cmd(repo_root);
    staged.args(["apply", "--index"]).arg(&backup);
    let label = if run(staged, "git apply", LOCAL_TIMEOUT).await.is_ok() {
        tr("Discarded changes restored and staged")
    } else {
        let mut plain = git_cmd(repo_root);
        plain.arg("apply").arg(&backup);
        run(plain, "git apply", LOCAL_TIMEOUT).await?;
        tr("Discarded changes restored (not staged)")
    };
    *LAST_DISCARD.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(label)
}

/// Name of the checked-out branch, `None` on a detached HEAD.
async fn current_branch(repo_root: &Path) -> Result<Option<String>, String> {
    let mut cmd = git_cmd(repo_root);
    cmd.args(["symbolic-ref", "--short", "-q", "HEAD"]);
    let out = cmd
        .output()
        .await
        .map_err(|e| format!("{}: {e}", tr("git symbolic-ref spawn failed")))?;
    if !out.status.success() {
        return Ok(None);
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok((!name.is_empty()).then_some(name))
}

/// Local branches and the checked-out one (`None` on a detached HEAD).
pub struct BranchList {
    pub current: Option<String>,
    pub branches: Vec<String>,
}

pub async fn list_branches(repo_root: &Path) -> Result<BranchList, String> {
    crate::hit!("list_branches");
    let mut cmd = git_cmd(repo_root);
    cmd.args(["branch", "--list", "--format=%(HEAD)%(refname:short)"]);
    let out = run(cmd, "git branch", LOCAL_TIMEOUT).await?;

    let mut current = None;
    let mut branches = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        // `%(HEAD)` is a single `*` for the checked-out branch and a space otherwise.
        let Some(mark) = line.chars().next() else {
            continue;
        };
        let name = line[mark.len_utf8()..].trim();
        if name.is_empty() || name.starts_with('(') {
            continue;
        }
        if mark == '*' {
            current = Some(name.to_string());
        }
        branches.push(name.to_string());
    }
    Ok(BranchList { current, branches })
}

fn checked_branch_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(tr("Branch name cannot be empty"));
    }
    if name.starts_with('-') {
        let msg = tr("'{}' is not a valid branch name").replace("{}", name);
        return Err(msg);
    }
    Ok(name)
}

pub async fn switch_branch(repo_root: &Path, name: String) -> Result<String, String> {
    crate::hit!("switch_branch");
    let name = checked_branch_name(&name)?;
    let mut cmd = git_cmd(repo_root);
    cmd.args(["switch", name]);
    run(cmd, "git switch", LOCAL_TIMEOUT).await?;
    let msg = tr("Switched to '{}'").replace("{}", name);
    Ok(msg)
}

pub async fn create_branch(repo_root: &Path, name: String) -> Result<String, String> {
    crate::hit!("create_branch");
    let name = checked_branch_name(&name)?;
    let mut check = git_cmd(repo_root);
    check.args(["check-ref-format", "--branch", name]);
    run(check, "git check-ref-format", LOCAL_TIMEOUT)
        .await
        .map_err(|_| tr("'{}' is not a valid branch name").replace("{}", name))?;
    let mut cmd = git_cmd(repo_root);
    cmd.args(["switch", "-c", name]);
    run(cmd, "git switch", LOCAL_TIMEOUT).await?;
    let msg = tr("Created and switched to '{}'").replace("{}", name);
    Ok(msg)
}

/// `git diff HEAD` as text.
pub async fn diff_head(repo_root: &Path) -> Result<String, String> {
    crate::hit!("diff_head");
    let mut cmd = git_cmd(repo_root);
    cmd.args(["diff", "HEAD", "--no-color", "--no-ext-diff"]);
    let out = run(cmd, "git diff", LOCAL_TIMEOUT).await?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if text.trim().is_empty() {
        return Err(tr("No changes to copy"));
    }
    Ok(text)
}

/// History of `paths` (relative to `repo_root`) as plain text, the whole repository when empty.
///
/// A single file is followed across renames.
pub async fn log_paths(repo_root: &Path, paths: &[PathBuf]) -> Result<String, String> {
    crate::hit!("log_paths");
    let mut cmd = git_cmd_literal(repo_root);
    cmd.args([
        "log",
        "-n",
        "300",
        "--no-color",
        "--date=short",
        "--format=%h  %ad  %an  %s%d",
    ]);
    if let [only] = paths {
        let is_file = tokio::fs::metadata(repo_root.join(only))
            .await
            .map(|m| m.is_file())
            .unwrap_or(false);
        if is_file {
            cmd.arg("--follow");
        }
    }
    if !paths.is_empty() {
        cmd.arg("--").args(paths);
    }
    let out = run(cmd, "git log", LOCAL_TIMEOUT).await?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if text.trim().is_empty() {
        return Err(tr("No commits found for this selection"));
    }
    Ok(text)
}

/// Percent-encodes everything except unreserved characters and `/`.
fn percent_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Builds the web URL of `reference` (a branch, or a commit when `detached`) for a git remote URL.
///
/// Understands `https://`, `ssh://`, `git://` and scp-like (`git@host:owner/repo`) remotes and
/// the URL layouts of GitHub, GitLab, Bitbucket and Gitea-style hosts. Credentials and ssh ports
/// are dropped. `rel` is an optional repo-relative path and whether it is a directory.
fn web_url(
    remote: &str,
    reference: &str,
    detached: bool,
    rel: Option<(&str, bool)>,
) -> Option<String> {
    let remote = remote.trim();
    let (scheme, rest) = match remote.split_once("://") {
        Some((scheme, rest)) => (Some(scheme), rest),
        None => (None, remote),
    };
    let (authority, path) = match scheme {
        Some(_) => rest.split_once('/')?,
        None => rest.split_once(':')?,
    };
    let host_port = authority.rsplit('@').next()?;
    let (web_scheme, host) = match scheme {
        Some(s @ ("http" | "https")) => (s, host_port),
        _ => ("https", host_port.split(':').next()?),
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if host.is_empty() || path.is_empty() {
        return None;
    }

    let h = host.to_ascii_lowercase();
    let (dir_seg, file_seg) = if h.contains("gitlab") {
        ("-/tree", "-/blob")
    } else if h.contains("bitbucket") {
        ("src", "src")
    } else if h.contains("codeberg") || h.contains("gitea") || h.contains("forgejo") {
        if detached {
            ("src/commit", "src/commit")
        } else {
            ("src/branch", "src/branch")
        }
    } else {
        ("tree", "blob")
    };

    let kind = match rel {
        Some((_, false)) => file_seg,
        _ => dir_seg,
    };
    let mut url = format!(
        "{web_scheme}://{host}/{path}/{kind}/{}",
        percent_encode_path(reference)
    );
    if let Some((rel, _)) = rel {
        if !rel.is_empty() {
            url.push('/');
            url.push_str(&percent_encode_path(rel));
        }
    }
    Some(url)
}

/// Opens the `origin` web page of the current branch, or of `target` (relative to `repo_root`) on it.
pub async fn open_remote(repo_root: &Path, target: Option<&Path>) -> Result<String, String> {
    crate::hit!("open_remote");
    let mut remote_cmd = git_cmd(repo_root);
    remote_cmd.args(["remote", "get-url", "origin"]);
    let remote = run(remote_cmd, "git remote", LOCAL_TIMEOUT)
        .await
        .map_err(|_| tr("No 'origin' remote configured"))?;
    let remote = String::from_utf8_lossy(&remote.stdout).trim().to_string();

    let (reference, detached) = match current_branch(repo_root).await? {
        Some(branch) => (branch, false),
        None => {
            let mut sha = git_cmd(repo_root);
            sha.args(["rev-parse", "HEAD"]);
            let out = run(sha, "git rev-parse", LOCAL_TIMEOUT).await?;
            (
                String::from_utf8_lossy(&out.stdout).trim().to_string(),
                true,
            )
        }
    };

    let rel = match target {
        Some(rel) if !rel.as_os_str().is_empty() => {
            let is_dir = tokio::fs::metadata(repo_root.join(rel))
                .await
                .map(|m| m.is_dir())
                .unwrap_or(false);
            Some((rel.to_string_lossy().into_owned(), is_dir))
        }
        _ => None,
    };

    let url = web_url(
        &remote,
        &reference,
        detached,
        rel.as_ref().map(|(p, d)| (p.as_str(), *d)),
    )
    .ok_or_else(|| tr("Cannot build a web URL from remote '{}'").replace("{}", &remote))?;

    Command::new("xdg-open")
        .arg(&url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {e}", tr("xdg-open spawn failed")))?;
    let msg = tr("Opening {}").replace("{}", &url);
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_url_from_common_remotes() {
        let want = "https://github.com/o/r/tree/main";
        for remote in [
            "git@github.com:o/r.git",
            "https://github.com/o/r.git",
            "https://user:token@github.com/o/r",
            "ssh://git@github.com:22/o/r.git",
            "git://github.com/o/r.git/",
        ] {
            assert_eq!(web_url(remote, "main", false, None).as_deref(), Some(want));
        }
    }

    #[test]
    fn web_url_file_and_directory_layouts() {
        assert_eq!(
            web_url(
                "git@github.com:o/r.git",
                "main",
                false,
                Some(("src/a b.rs", false))
            )
            .as_deref(),
            Some("https://github.com/o/r/blob/main/src/a%20b.rs")
        );
        assert_eq!(
            web_url(
                "git@gitlab.com:g/s/r.git",
                "dev",
                false,
                Some(("src", true))
            )
            .as_deref(),
            Some("https://gitlab.com/g/s/r/-/tree/dev/src")
        );
        assert_eq!(
            web_url("https://codeberg.org/o/r", "abc123", true, None).as_deref(),
            Some("https://codeberg.org/o/r/src/commit/abc123")
        );
        assert_eq!(
            web_url("https://git.example.com:8443/o/r.git", "x", false, None).as_deref(),
            Some("https://git.example.com:8443/o/r/tree/x")
        );
    }

    #[test]
    fn web_url_rejects_local_remotes() {
        assert_eq!(web_url("/srv/git/repo.git", "main", false, None), None);
        assert_eq!(web_url("", "main", false, None), None);
    }

    #[test]
    fn repo_relative_drops_root_and_outsiders() {
        let root = Path::new("/r");
        let got = repo_relative(
            root,
            vec!["/r".into(), "/r/a/b".into(), "/elsewhere/c".into()],
        );
        assert_eq!(got, vec![PathBuf::from("a/b")]);
    }
}
