//! Read-only probes: is this a LUKS file, is it already attached, is it already
//! mounted.
//!
//! Every function here either shells out to `file` (which reads magic bytes)
//! or reads `/sys/block` and `/proc/mounts`. Nothing mutates system state.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Returns `true` if the magic bytes of `path` identify it as a LUKS container.
pub fn is_luks_image(path: &Path) -> bool {
    Command::new("file")
        .args(["-b", &path.to_string_lossy()])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("LUKS encrypted file"))
        .unwrap_or(false)
}

/// Reads `/proc/mounts` to find the current mount point of a dm-crypt device
/// by name (i.e. `/dev/mapper/<name>`).
pub fn find_mount_point(device_name: &str) -> Option<PathBuf> {
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    mounts.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let dev = parts.next()?;
        let mount = parts.next()?;
        if dev == format!("/dev/mapper/{device_name}") {
            Some(PathBuf::from(mount))
        } else {
            None
        }
    })
}

/// Walks `/sys/block` looking for a `loopN` device whose backing file is
/// `image_path`, and returns `(loop_dev, dm_dev)` if found.
///
/// `dm_dev` is empty when the loop is attached but the LUKS layer hasn't been
/// unlocked yet.
pub(super) fn find_existing_luks_setup(image_path: &Path) -> Option<(String, String)> {
    let canonical = image_path.canonicalize().ok()?;
    let entries = std::fs::read_dir("/sys/block").ok()?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("loop") {
            continue;
        }

        if let Ok(backing) =
            std::fs::read_to_string(format!("/sys/block/{name_str}/loop/backing_file"))
        {
            if Path::new(backing.trim()) == canonical {
                let loop_dev = format!("/dev/{name_str}");

                let holders_dir = format!("/sys/block/{name_str}/holders");
                if let Ok(holders) = std::fs::read_dir(holders_dir) {
                    if let Some(holder) = holders.flatten().next() {
                        let dm_name = holder.file_name().to_string_lossy().to_string();
                        let dm_dev = format!("/dev/{dm_name}");
                        return Some((loop_dev, dm_dev));
                    }
                }
                return Some((loop_dev, String::new()));
            }
        }
    }
    None
}

/// Checks `/proc/mounts` to see if a `/dev/dm-X` device node is already mounted.
pub(super) fn get_mount_point_for_dm(dm_dev: &str) -> Option<PathBuf> {
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    for line in mounts.lines() {
        let mut parts = line.split_whitespace();
        let dev = parts.next()?;
        let mount = parts.next()?;
        if dev == dm_dev {
            return Some(PathBuf::from(mount));
        }
    }
    None
}
