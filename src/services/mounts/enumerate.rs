use super::naming::resolve_mount_name;
use std::fs;
use std::path::PathBuf;

/// Returns `(display_name, path)` pairs for every mount point the sidebar
/// should show: removable media under `/media` and `/run/media`, `/mnt/*`
/// entries, FUSE mounts inside the user's home, and any active network
/// mounts reported by [`crate::services::network`].
pub fn get_system_mounts() -> Vec<(String, PathBuf)> {
    let mut mounts = Vec::new();
    let home_dir = dirs::home_dir().unwrap_or_default();

    let mut media_roots = vec![PathBuf::from("/media")];
    if let Ok(user) = std::env::var("USER") {
        media_roots.push(PathBuf::from(format!("/run/media/{}", user)));
    }
    if let Ok(entries) = fs::read_dir("/run/media") {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && !media_roots.contains(&p) {
                media_roots.push(p);
            }
        }
    }

    for root in media_roots {
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path != home_dir {
                    let name = resolve_mount_name(&path, None);
                    if !mounts.iter().any(|(_, p)| p == &path) {
                        mounts.push((name, path));
                    }
                }
            }
        }
    }

    if let Ok(content) = fs::read_to_string("/proc/self/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }
            let dev_node = parts[0];
            let path_str = parts[1];
            let fs_type = parts[2];
            let path = PathBuf::from(path_str);

            if path_str == "/"
                || path_str == "/home"
                || path_str == "/var/home"
                || path == home_dir
                || (path_str.starts_with("/home/") && path.components().count() <= 3)
                || path_str.starts_with("/app")
                || path_str.starts_with("/run/flatpak")
            {
                continue;
            }

            let is_mnt = path_str.starts_with("/mnt/") && path_str != "/mnt";
            let is_user_fuse =
                fs_type.contains("fuse") && path.starts_with(&home_dir) && path != home_dir;

            if is_mnt || is_user_fuse {
                let name = resolve_mount_name(&path, Some(dev_node));
                if !mounts.iter().any(|(_, p)| p == &path) {
                    mounts.push((name, path));
                }
            }
        }
    }

    for (uri, name, _icon) in crate::services::network::active_mounts() {
        let path = PathBuf::from(uri);
        if path.is_absolute() && !mounts.iter().any(|(_, p)| p == &path) {
            mounts.push((name, path));
        }
    }

    mounts
}
