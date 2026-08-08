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
