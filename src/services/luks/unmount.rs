//! LUKS unmount + lock + loop cleanup.

use std::process::Command;

/// Unmounts and closes a LUKS volume, then deletes the loop device.
///
/// Runs blocking - must be called from `relm4::spawn_blocking`.
#[allow(dead_code)]
pub fn unmount_and_lock(device_name: &str) -> Result<(), String> {
    let dm_dev = format!("/dev/mapper/{device_name}");

    let unmount_out = Command::new("udisksctl")
        .args(["unmount", "-b", &dm_dev, "--no-user-interaction"])
        .output()
        .map_err(|e| format!("udisksctl unmount failed: {e}"))?;

    if !unmount_out.status.success() {
        return Err(String::from_utf8_lossy(&unmount_out.stderr).to_string());
    }

    let _ = Command::new("udisksctl")
        .args(["lock", "-b", &dm_dev])
        .output();

    // Best-effort loop cleanup - find and delete the backing loop device.
    if let Ok(entries) = std::fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("loop") {
                continue;
            }
            let backing = std::fs::read_to_string(format!("/sys/block/{name}/loop/backing_file"));
            if backing.is_err() {
                continue;
            }
            let loop_dev = format!("/dev/{name}");
            let _ = Command::new("udisksctl")
                .args(["loop-delete", "-b", &loop_dev])
                .output();
        }
    }

    Ok(())
}
