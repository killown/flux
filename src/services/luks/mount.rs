//! LUKS unlock + mount sequence.

use super::image::LuksImage;
use super::probe::{find_existing_luks_setup, find_mount_point, get_mount_point_for_dm};
use std::path::PathBuf;
use std::process::Command;

/// Loop-attaches `image`, unlocks it via `udisksctl unlock --key-file` (no
/// polkit agent required), sets the filesystem label to the image stem so
/// udisksctl mounts it at `/run/media/<user>/<name>` instead of a UUID path,
/// then mounts the dm node via `udisksctl mount`.
///
/// The passphrase is written to a `NamedTempFile` which is deleted immediately
/// after the unlock handshake completes - it never persists on disk.
///
/// If the volume is already mounted, returns the existing mount point immediately.
/// Runs blocking - must be called from `relm4::spawn_blocking`.
pub fn unlock_and_mount(image: &LuksImage, passphrase: &str) -> Result<PathBuf, String> {
    let device_name = image
        .path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("flux_luks")
        .to_string();

    // Short-circuit: already mounted by another tool.
    if let Some(mount_point) = find_mount_point(&device_name) {
        return Ok(mount_point);
    }

    // Reuse existing loop attachment / unlocked device mapper node if already present.
    if let Some((_existing_loop, dm_dev)) = find_existing_luks_setup(&image.path) {
        if !dm_dev.is_empty() {
            if let Some(mount_point) = get_mount_point_for_dm(&dm_dev) {
                return Ok(mount_point);
            }

            let mount_out = Command::new("udisksctl")
                .args(["mount", "-b", &dm_dev, "--no-user-interaction"])
                .output()
                .map_err(|e| format!("udisksctl mount failed: {e}"))?;

            if mount_out.status.success() {
                let mount_stdout = String::from_utf8_lossy(&mount_out.stdout);
                if let Some(mount_point) = mount_stdout
                    .split(" at ")
                    .nth(1)
                    .map(|s| s.trim().trim_end_matches('.'))
                {
                    return Ok(PathBuf::from(mount_point));
                }
            }
        }
    }

    // 1. Loop-attach - needs no privileges.
    let loop_out = Command::new("udisksctl")
        .args(["loop-setup", "-f", &image.path.to_string_lossy()])
        .output()
        .map_err(|e| format!("udisksctl loop-setup failed: {e}"))?;

    if !loop_out.status.success() {
        return Err(String::from_utf8_lossy(&loop_out.stderr).to_string());
    }

    let loop_stdout = String::from_utf8_lossy(&loop_out.stdout);
    let loop_dev = loop_stdout
        .split_whitespace()
        .last()
        .map(|s| s.trim_end_matches('.'))
        .filter(|s| s.starts_with("/dev/loop"))
        .ok_or_else(|| format!("Could not parse loop device from: {loop_stdout}"))?
        .to_string();

    // 2. Write passphrase to a tempfile - udisksctl reads it without a TTY or
    //    polkit agent, delegating privilege to the udisks D-Bus daemon directly.
    let mut key_file = tempfile::NamedTempFile::new()
        .map_err(|e| format!("Failed to create key tempfile: {e}"))?;
    {
        use std::io::Write;
        key_file
            .write_all(passphrase.as_bytes())
            .map_err(|e| format!("Failed to write key tempfile: {e}"))?;
    }

    let unlock_out = Command::new("udisksctl")
        .args([
            "unlock",
            "-b",
            &loop_dev,
            "--key-file",
            &key_file.path().to_string_lossy(),
        ])
        .output()
        .map_err(|e| format!("udisksctl unlock failed: {e}"))?;

    // Passphrase tempfile is dropped and deleted here regardless of outcome.
    drop(key_file);

    if !unlock_out.status.success() {
        let _ = Command::new("udisksctl")
            .args(["loop-delete", "-b", &loop_dev])
            .output();
        let stderr = String::from_utf8_lossy(&unlock_out.stderr).to_lowercase();
        return Err(
            if stderr.contains("bad passphrase") || stderr.contains("no key available") {
                crate::i18n::tr("Wrong passphrase.")
            } else {
                String::from_utf8_lossy(&unlock_out.stderr).to_string()
            },
        );
    }

    // Output: "Unlocked /dev/loopN as /dev/dm-N."
    let unlock_stdout = String::from_utf8_lossy(&unlock_out.stdout);
    let dm_dev = unlock_stdout
        .split_whitespace()
        .last()
        .map(|s| s.trim_end_matches('.'))
        .filter(|s| s.starts_with("/dev/dm-") || s.starts_with("/dev/mapper/"))
        .ok_or_else(|| format!("Could not parse dm device from: {unlock_stdout}"))?
        .to_string();

    // 3. Set filesystem label across common filesystems so udisksctl mounts at
    //    /run/media/<user>/<device_name> instead of the UUID-based path.
    //    Best-effort execution across multiple tools.
    let _ = Command::new("e2label")
        .args([&dm_dev, &device_name])
        .output();

    let _ = Command::new("fatlabel")
        .args([&dm_dev, &device_name])
        .output();

    let _ = Command::new("exfatlabel")
        .args([&dm_dev, &device_name])
        .output();

    let _ = Command::new("btrfs")
        .args(["filesystem", "label", &dm_dev, &device_name])
        .output();

    let _ = Command::new("xfs_admin")
        .args(["-L", &device_name, &dm_dev])
        .output();

    // 4. Mount - no credentials needed once the dm node is open.
    let mount_out = Command::new("udisksctl")
        .args(["mount", "-b", &dm_dev, "--no-user-interaction"])
        .output()
        .map_err(|e| format!("udisksctl mount failed: {e}"))?;

    if !mount_out.status.success() {
        return Err(String::from_utf8_lossy(&mount_out.stderr).to_string());
    }

    let mount_stdout = String::from_utf8_lossy(&mount_out.stdout);
    let mount_point = mount_stdout
        .split(" at ")
        .nth(1)
        .map(|s| s.trim().trim_end_matches('.'))
        .ok_or_else(|| format!("Could not parse mount point from: {mount_stdout}"))?;

    Ok(PathBuf::from(mount_point))
}
