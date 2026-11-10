use super::naming::{resolve_luks_loop_name, resolve_mount_name};
use gtk::gio;
use gtk::gio::prelude::*;
use std::fs;
use std::path::PathBuf;

/// Returns `(display_name, path)` pairs for every mount point the sidebar
/// should show: removable media under `/media` and `/run/media`, `/mnt/*`
/// entries, FUSE mounts inside the user's home, and any active network
/// mounts reported by [`crate::infra::network`].
pub fn get_system_mounts() -> Vec<(String, PathBuf)> {
    crate::hit!("mounts::get_system_mounts");
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

            let is_luks_image = resolve_luks_loop_name(dev_node).is_some();

            if is_mnt || is_user_fuse || is_luks_image {
                let name = resolve_mount_name(&path, Some(dev_node));
                if !mounts.iter().any(|(_, p)| p == &path) {
                    mounts.push((name, path));
                }
            }
        }
    }

    for (uri, name, _icon) in crate::infra::network::active_mounts() {
        let path = PathBuf::from(uri);
        if path.is_absolute() && !mounts.iter().any(|(_, p)| p == &path) {
            mounts.push((name, path));
        }
    }

    mounts
}

/// A volume that is present but not mounted, as the sidebar lists it when
/// `ui.show_unmounted_volumes` is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmountedVolume {
    /// Lookup key for [`find_unmounted_volume`]: the `unix-device` identifier,
    /// else the UUID, else the display name.
    pub id: String,
    pub name: String,
    pub icon: String,
}

fn volume_id(volume: &gio::Volume) -> String {
    volume
        .identifier("unix-device")
        .or_else(|| volume.uuid())
        .map(|s| s.to_string())
        .unwrap_or_else(|| volume.name().to_string())
}

/// Lists the volumes the volume monitor knows about that have no mount and can
/// be mounted. Reads the monitor's cached state, so it does no I/O, call it on
/// the GTK main thread because `gio::Volume` is not `Send`.
pub fn unmounted_volumes() -> Vec<UnmountedVolume> {
    crate::hit!("mounts::unmounted_volumes");
    gio::VolumeMonitor::get()
        .volumes()
        .into_iter()
        .filter(|v| v.get_mount().is_none() && v.can_mount())
        .filter_map(|v| {
            let name = v.name().to_string();
            if name.is_empty() {
                return None;
            }
            let icon = v
                .icon()
                .downcast::<gio::ThemedIcon>()
                .ok()
                .and_then(|themed| themed.names().first().map(|s| s.to_string()))
                .unwrap_or_else(|| "drive-harddisk-symbolic".to_owned());
            Some(UnmountedVolume {
                id: volume_id(&v),
                name,
                icon,
            })
        })
        .collect()
}

/// Finds the volume that [`unmounted_volumes`] reported under `id`.
pub fn find_unmounted_volume(id: &str) -> Option<gio::Volume> {
    crate::hit!("mounts::find_unmounted_volume");
    gio::VolumeMonitor::get()
        .volumes()
        .into_iter()
        .find(|v| volume_id(v) == id)
}
