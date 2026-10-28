//! LUKS unlock + mount sequence.

use super::image::LuksImage;
use super::parse::{is_wrong_passphrase, parse_dm_device, parse_loop_device, parse_mount_point};
use super::probe::{find_existing_luks_setup, find_mount_point, get_mount_point_for_dm};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Builds a `udisksctl` invocation pinned to the C locale.
///
/// The output parsers match English wording, so without `LC_ALL=C` a translated
/// `udisksctl` (e.g. pt_BR) makes a successful mount look like a parse failure.
fn udisksctl_command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.env("LC_ALL", "C");
    cmd
}

/// Undoes the loop attachment, the unlock and the mount made by one
/// `unlock_and_mount_with` call when it returns early.
///
/// Armed on creation and disarmed only once the mount point is known, so every
/// `?`/`return Err` between loop-setup and success tears down whatever was
/// created so far. Teardown order is unmount, lock, loop-delete.
struct SetupGuard<'a> {
    udisksctl: &'a Path,
    loop_dev: String,
    unlocked: bool,
    mounted_dev: Option<String>,
    armed: bool,
}

impl<'a> SetupGuard<'a> {
    /// Arms a guard for the freshly attached `loop_dev`.
    fn new(udisksctl: &'a Path, loop_dev: &str) -> Self {
        Self {
            udisksctl,
            loop_dev: loop_dev.to_string(),
            unlocked: false,
            mounted_dev: None,
            armed: true,
        }
    }

    /// Keeps the setup alive: the guard no longer tears anything down.
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for SetupGuard<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if let Some(dm_dev) = &self.mounted_dev {
            let _ = udisksctl_command(self.udisksctl)
                .args(["unmount", "-b", dm_dev, "--no-user-interaction"])
                .output();
        }
        if self.unlocked {
            let _ = udisksctl_command(self.udisksctl)
                .args(["lock", "-b", &self.loop_dev])
                .output();
        }
        let _ = udisksctl_command(self.udisksctl)
            .args(["loop-delete", "-b", &self.loop_dev])
            .output();
    }
}

/// Loop-attaches `image`, unlocks it via `udisksctl unlock --key-file` (no
/// polkit agent required), then mounts the dm node via `udisksctl mount`.
///
/// The passphrase is written to a `NamedTempFile` which is deleted immediately
/// after the unlock handshake completes - it never persists on disk.
///
/// If the volume is already mounted, returns the existing mount point immediately.
/// If any step after the loop attachment fails, the mount, the unlock and the
/// loop device created by this call are rolled back.
/// Runs blocking - must be called from `relm4::spawn_blocking`.
pub fn unlock_and_mount(image: &LuksImage, passphrase: &str) -> Result<PathBuf, String> {
    crate::hit!("unlock_and_mount");
    unlock_and_mount_with(Path::new("udisksctl"), image, passphrase)
}

/// Same as [`unlock_and_mount`], but runs the `udisksctl` binary at `udisksctl`
/// instead of resolving it through `PATH`.
pub fn unlock_and_mount_with(
    udisksctl: &Path,
    image: &LuksImage,
    passphrase: &str,
) -> Result<PathBuf, String> {
    crate::hit!("unlock_and_mount_with");
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

            let mount_out = udisksctl_command(udisksctl)
                .args(["mount", "-b", &dm_dev, "--no-user-interaction"])
                .output()
                .map_err(|e| format!("udisksctl mount failed: {e}"))?;

            if mount_out.status.success() {
                let mount_stdout = String::from_utf8_lossy(&mount_out.stdout);
                if let Some(mount_point) = parse_mount_point(&mount_stdout) {
                    return Ok(mount_point);
                }
            }
        }
    }

    // 1. Loop-attach - needs no privileges.
    let loop_out = udisksctl_command(udisksctl)
        .args(["loop-setup", "-f", &image.path.to_string_lossy()])
        .output()
        .map_err(|e| format!("udisksctl loop-setup failed: {e}"))?;

    if !loop_out.status.success() {
        return Err(String::from_utf8_lossy(&loop_out.stderr).to_string());
    }

    let loop_stdout = String::from_utf8_lossy(&loop_out.stdout);
    let loop_dev = parse_loop_device(&loop_stdout)
        .ok_or_else(|| format!("Could not parse loop device from: {loop_stdout}"))?;

    let mut guard = SetupGuard::new(udisksctl, &loop_dev);

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

    let unlock_out = udisksctl_command(udisksctl)
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
        let stderr = String::from_utf8_lossy(&unlock_out.stderr);
        return Err(if is_wrong_passphrase(&stderr) {
            crate::i18n::tr("Wrong passphrase.")
        } else {
            stderr.to_string()
        });
    }
    guard.unlocked = true;

    // Output: "Unlocked /dev/loopN as /dev/dm-N."
    let unlock_stdout = String::from_utf8_lossy(&unlock_out.stdout);
    let dm_dev = parse_dm_device(&unlock_stdout)
        .ok_or_else(|| format!("Could not parse dm device from: {unlock_stdout}"))?;

    // 3. Mount - no credentials needed once the dm node is open.
    let mount_out = udisksctl_command(udisksctl)
        .args(["mount", "-b", &dm_dev, "--no-user-interaction"])
        .output()
        .map_err(|e| format!("udisksctl mount failed: {e}"))?;

    if !mount_out.status.success() {
        return Err(String::from_utf8_lossy(&mount_out.stderr).to_string());
    }
    guard.mounted_dev = Some(dm_dev.clone());

    let mount_stdout = String::from_utf8_lossy(&mount_out.stdout);
    let mount_point = parse_mount_point(&mount_stdout)
        .or_else(|| get_mount_point_for_dm(&dm_dev))
        .ok_or_else(|| format!("Could not parse mount point from: {mount_stdout}"))?;

    guard.disarm();
    Ok(mount_point)
}
