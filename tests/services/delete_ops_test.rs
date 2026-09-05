use std::path::Path;

#[allow(dead_code)]
fn is_protected_target(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
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
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if resolved == Path::new("/") {
        return true;
    }
    if let Some(home) = dirs::home_dir() {
        if let Ok(canon_home) = home.canonicalize() {
            if resolved == canon_home {
                return true;
            }
        } else if resolved == home {
            return true;
        }
    }
    let protected_system_paths = [
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
    for sys_path in protected_system_paths {
        if resolved == Path::new(sys_path) {
            return true;
        }
    }
    false
}
