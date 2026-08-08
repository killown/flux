use flux::services::luks::{find_mount_point, is_luks_image, LuksImage};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[test]
fn test_is_luks_image_non_luks_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("plain_text.txt");
    let mut file = File::create(&file_path).unwrap();
    writeln!(file, "This is a plain text file, not a LUKS container.").unwrap();

    assert!(!is_luks_image(&file_path));
}

#[test]
fn test_is_luks_image_non_existent_file() {
    let missing_path = Path::new("/nonexistent/path/to/vault.img");
    assert!(!is_luks_image(missing_path));
}

#[test]
fn test_luks_image_struct_creation() {
    let path = PathBuf::from("/tmp/test-vault.img");
    let image = LuksImage { path: path.clone() };

    assert_eq!(image.path, path);
}

#[test]
fn test_find_mount_point_non_existent_device() {
    let mount = find_mount_point("non_existent_device_mapper_xyz_123");
    assert!(mount.is_none());
}

#[test]
fn test_find_mount_point_empty_device_name() {
    let mount = find_mount_point("");
    assert!(mount.is_none());
}

#[test]
fn test_find_mount_point_dm_device_formatting() {
    let mock_proc_mounts = "\
/dev/mapper/test_vault /run/media/user/test_vault ext4 rw 0 0
/dev/dm-0 /run/media/user/secret_disk ext4 rw 0 0";

    let parse_dm = |mounts: &str, dev_name: &str| -> Option<PathBuf> {
        mounts.lines().find_map(|line| {
            let mut parts = line.split_whitespace();
            let dev = parts.next()?;
            let mount = parts.next()?;
            if dev == format!("/dev/mapper/{dev_name}") || dev == dev_name {
                Some(PathBuf::from(mount))
            } else {
                None
            }
        })
    };

    assert_eq!(
        parse_dm(mock_proc_mounts, "test_vault"),
        Some(PathBuf::from("/run/media/user/test_vault"))
    );
    assert_eq!(
        parse_dm(mock_proc_mounts, "/dev/dm-0"),
        Some(PathBuf::from("/run/media/user/secret_disk"))
    );
}

#[test]
fn test_luks_stderr_passphrase_matching() {
    let bad_passphrase_stderr = "Error unlocking /dev/loop0: GDBus.Error:org.freedesktop.UDisks2.Error.Failed: Operation failed: bad passphrase";
    let no_key_stderr = "GDBus.Error:org.freedesktop.UDisks2.Error.Failed: no key available";

    let check_wrong_passphrase = |stderr: &str| -> bool {
        let stderr_lc = stderr.to_lowercase();
        stderr_lc.contains("bad passphrase") || stderr_lc.contains("no key available")
    };

    assert!(check_wrong_passphrase(bad_passphrase_stderr));
    assert!(check_wrong_passphrase(no_key_stderr));
    assert!(!check_wrong_passphrase("Device busy"));
}
