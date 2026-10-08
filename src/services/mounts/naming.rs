use std::fs;
use std::path::Path;
use std::process::Command;

/// Resolves a device-mapper node (`/dev/dm-N` or `/dev/mapper/...`) to the
/// file stem of its backing loop file, when it's a LUKS image.
fn resolve_luks_loop_name(dev_node: &str) -> Option<String> {
    let dev_path = Path::new(dev_node);
    let dev_name = dev_path.file_name()?.to_str()?;
    if !dev_name.starts_with("dm-") && !dev_name.starts_with("mapper/") {
        return None;
    }
    let sys_path = format!("/sys/block/{}/slaves", dev_name);
    if let Ok(entries) = fs::read_dir(sys_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("loop") {
                let backing = format!("/sys/block/{}/loop/backing_file", name);
                if let Ok(backing_path) = fs::read_to_string(backing) {
                    let backing_path = backing_path.trim();
                    if let Some(file_name) = Path::new(backing_path).file_stem() {
                        if let Some(s) = file_name.to_str() {
                            return Some(s.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Queries the filesystem label of a device with `lsblk`.
fn get_fs_label(dev_node: &str) -> Option<String> {
    let output = Command::new("lsblk")
        .args(["-no", "LABEL", dev_node])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let label = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if label.is_empty() {
        None
    } else {
        Some(label)
    }
}

/// Produces a human-readable name for a mount at `path`.
///
/// Tries, in order:
///   1. the LUKS backing file stem, if the device is a dm-crypt mapping,
///   2. the filesystem label via `lsblk`,
///   3. the last non-`home`, non-`$USER` component of `path`,
///   4. the device basename,
///   5. the literal string `"Mount"`.
pub(super) fn resolve_mount_name(path: &Path, dev_node: Option<&str>) -> String {
    let user_name = std::env::var("USER").unwrap_or_default();

    if let Some(dev) = dev_node {
        if let Some(luks) = resolve_luks_loop_name(dev) {
            return luks;
        }
        if let Some(label) = get_fs_label(dev) {
            return label;
        }
    }

    let components: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    for &comp in components.iter().rev() {
        if !comp.eq_ignore_ascii_case("home") && !comp.eq_ignore_ascii_case(&user_name) {
            return comp.trim_start_matches('.').to_string();
        }
    }

    if let Some(dev) = dev_node {
        if let Some(base) = Path::new(dev).file_name().and_then(|n| n.to_str()) {
            return base.to_string();
        }
    }
    "Mount".to_string()
}
