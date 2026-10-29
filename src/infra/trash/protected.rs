use std::path::{Path, PathBuf};

/// Resolves `path` for a protection check without following a symlink in its
/// last component.
pub fn resolve_entry(path: &Path) -> PathBuf {
    crate::hit!("resolve_entry");
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        if let Ok(canon_parent) = parent.canonicalize() {
            return canon_parent.join(name);
        }
    }
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Returns `true` when `path` must never be deleted by the UI.
pub fn is_protected_target(path: &Path) -> bool {
    crate::hit!("is_protected_target");
    let path_str = path.to_string_lossy();

    // Check raw URI root targets.
    if path_str.contains("://") {
        if let Some((_, after_scheme)) = path_str.split_once("://") {
            let inner_path = after_scheme
                .find('/')
                .map(|i| &after_scheme[i..])
                .unwrap_or("/");
            if inner_path.is_empty() || inner_path == "/" {
                return true;
            }
        }
        return false;
    }

    // Standard path evaluation. The parent directory is resolved but a symlink
    // in the last component is not followed, deleting a link only removes the
    // link, so `~/mnt -> /mnt` must not be treated as `/mnt` itself.
    let resolved = resolve_entry(path);

    if resolved == Path::new("/") {
        return true;
    }

    if let Some(home) = dirs::home_dir() {
        if resolved == resolve_entry(&home) {
            return true;
        }
        if let Ok(canon_home) = home.canonicalize() {
            if resolved == canon_home {
                return true;
            }
        }
    }

    const PROTECTED: &[&str] = &[
        "/boot",
        "/dev",
        "/etc",
        "/lost+found",
        "/media",
        "/mnt",
        "/proc",
        "/root",
        "/run",
        "/run/media",
        "/sys",
        "/tmp",
        "/usr",
        "/var",
    ];

    PROTECTED.iter().any(|p| resolved == Path::new(p))
}
