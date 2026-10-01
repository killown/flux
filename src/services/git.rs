use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

static HAS_NERD_FONT: OnceLock<bool> = OnceLock::new();

/// Returns whether a Nerd Font is available on the system using Fontconfig.
pub fn is_nerd_font_available() -> bool {
    *HAS_NERD_FONT.get_or_init(|| {
        #[link(name = "fontconfig")]
        extern "C" {
            fn FcInitLoadConfigAndFonts() -> *mut std::ffi::c_void;
            fn FcPatternCreate() -> *mut std::ffi::c_void;
            fn FcPatternDestroy(p: *mut std::ffi::c_void);
            fn FcConfigDestroy(c: *mut std::ffi::c_void);
            fn FcFontList(
                config: *mut std::ffi::c_void,
                p: *mut std::ffi::c_void,
                os: *mut std::ffi::c_void,
            ) -> *mut std::ffi::c_void;
            fn FcFontSetDestroy(fs: *mut std::ffi::c_void);
            fn FcPatternGetString(
                p: *mut std::ffi::c_void,
                object: *const std::os::raw::c_char,
                n: std::os::raw::c_int,
                s: *mut *mut std::os::raw::c_char,
            ) -> std::os::raw::c_int;
        }

        #[repr(C)]
        struct FcFontSet {
            nfont: std::os::raw::c_int,
            sfont: std::os::raw::c_int,
            fonts: *mut *mut std::ffi::c_void,
        }

        unsafe {
            let config = FcInitLoadConfigAndFonts();
            if config.is_null() {
                return false;
            }

            let pat = FcPatternCreate();
            let fs_ptr = FcFontList(config, pat, std::ptr::null_mut());
            FcPatternDestroy(pat);

            let mut found = false;
            if !fs_ptr.is_null() {
                let fs = &*(fs_ptr as *const FcFontSet);
                let family_key = c"family";
                for i in 0..fs.nfont {
                    let font_pat = *fs.fonts.add(i as usize);
                    let mut family_ptr: *mut std::os::raw::c_char = std::ptr::null_mut();
                    if FcPatternGetString(font_pat, family_key.as_ptr(), 0, &mut family_ptr) == 0
                        && !family_ptr.is_null()
                    {
                        let family = std::ffi::CStr::from_ptr(family_ptr).to_string_lossy();
                        if family.contains("Nerd Font") || family.contains("Symbols Nerd") {
                            found = true;
                            break;
                        }
                    }
                }
                FcFontSetDestroy(fs_ptr);
            }
            FcConfigDestroy(config);
            found
        }
    })
}

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

    /// Returns display badge emblem and CSS class name.
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

/// Finds `.git` repository root by traversing parent paths.
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

/// Adds a rolled-up status for each direct child of `target_dir` holding changes, exact entries win.
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

/// Scans git status for `target_dir` and returns exact entries plus per-child directory rollups.
pub async fn query_git_status_for_view(
    repo_root: &Path,
    target_dir: &Path,
) -> Result<HashMap<PathBuf, GitFileStatus>, String> {
    let raw = query_git_status(repo_root, target_dir).await?;
    Ok(with_directory_rollup(target_dir, raw))
}
