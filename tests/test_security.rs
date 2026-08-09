use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::tempdir;

use adw::gio::prelude::*;
use flux::ui::paste_ops::perform_file_op;
use gtk::gio;

#[test]
fn test_security_cancellation_preserves_source_and_cleans_destination() {
    let src_dir = tempdir().unwrap();
    let dest_dir = tempdir().unwrap();

    let src_path = src_dir.path().join("unmodified_source.bin");
    let mut file = File::create(&src_path).unwrap();
    file.write_all(&[0xAAu8; 8192]).unwrap();

    let dest_path = dest_dir.path().join("unmodified_source.bin");

    File::create(&dest_path)
        .unwrap()
        .write_all(&[0x00u8; 1024])
        .unwrap();

    let cancellable = gio::Cancellable::new();
    cancellable.cancel();

    let res = perform_file_op(&src_path, &dest_path, true, &cancellable);

    assert!(res.is_err());
    assert!(src_path.exists());
    assert!(!dest_path.exists());
}

#[test]
fn test_security_successful_cut_deferred_source_deletion() {
    let src_dir = tempdir().unwrap();
    let dest_dir = tempdir().unwrap();

    let src_path = src_dir.path().join("cut_target.txt");
    let mut file = File::create(&src_path).unwrap();
    file.write_all(b"Verified Transfer Content").unwrap();

    let dest_path = dest_dir.path().join("cut_target.txt");
    let cancellable = gio::Cancellable::new();

    let res = perform_file_op(&src_path, &dest_path, true, &cancellable);

    assert!(res.is_ok());
    assert!(!src_path.exists());
    assert!(dest_path.exists());
    assert_eq!(
        fs::read_to_string(&dest_path).unwrap(),
        "Verified Transfer Content"
    );
}

#[test]
fn test_security_non_existent_source_does_not_create_garbage() {
    let dest_dir = tempdir().unwrap();
    let src_path = PathBuf::from("/tmp/non_existent_flux_test_file_999.bin");
    let dest_path = dest_dir.path().join("output.bin");

    let cancellable = gio::Cancellable::new();
    let res = perform_file_op(&src_path, &dest_path, true, &cancellable);

    assert!(res.is_err());
    assert!(!dest_path.exists());
}

#[test]
fn test_security_recursive_directory_cancellation_preserves_tree() {
    let src_dir = tempdir().unwrap();
    let dest_dir = tempdir().unwrap();

    let root = src_dir.path().join("folder_to_cut");
    let sub = root.join("subfolder");
    fs::create_dir_all(&sub).unwrap();

    let f1 = root.join("file1.bin");
    let f2 = sub.join("file2.bin");

    File::create(&f1).unwrap().write_all(&[0x11; 512]).unwrap();
    File::create(&f2).unwrap().write_all(&[0x22; 512]).unwrap();

    let dest_folder = dest_dir.path().join("folder_to_cut");

    let cancellable = gio::Cancellable::new();
    cancellable.cancel();

    let res = perform_file_op(&root, &dest_folder, true, &cancellable);

    assert!(res.is_err());
    assert!(root.exists());
    assert!(sub.exists());
    assert!(f1.exists());
    assert!(f2.exists());
    assert!(!dest_folder.exists());
}

#[test]
fn test_security_read_only_destination_preserves_source() {
    let src_dir = tempdir().unwrap();
    let dest_dir = tempdir().unwrap();

    let src_path = src_dir.path().join("protected_file.txt");
    File::create(&src_path).unwrap().write_all(b"data").unwrap();

    let readonly_dest_dir = dest_dir.path().join("readonly_folder");
    fs::create_dir(&readonly_dest_dir).unwrap();

    let mut permissions = fs::metadata(&readonly_dest_dir).unwrap().permissions();
    permissions.set_mode(0o444);
    let _ = fs::set_permissions(&readonly_dest_dir, permissions);

    let dest_path = readonly_dest_dir.join("protected_file.txt");
    let cancellable = gio::Cancellable::new();

    let res = perform_file_op(&src_path, &dest_path, true, &cancellable);

    let mut reset_perms = fs::metadata(&readonly_dest_dir).unwrap().permissions();
    reset_perms.set_mode(0o755);
    let _ = fs::set_permissions(&readonly_dest_dir, reset_perms);

    assert!(res.is_err());
    assert!(src_path.exists());
    assert!(!dest_path.exists());
}

#[test]
fn test_security_copy_mode_preserves_source_on_success() {
    let src_dir = tempdir().unwrap();
    let dest_dir = tempdir().unwrap();

    let src_path = src_dir.path().join("copy_source.bin");
    File::create(&src_path)
        .unwrap()
        .write_all(&[0xBB; 2048])
        .unwrap();

    let dest_path = dest_dir.path().join("copy_dest.bin");
    let cancellable = gio::Cancellable::new();

    let res = perform_file_op(&src_path, &dest_path, false, &cancellable);

    assert!(res.is_ok());
    assert!(src_path.exists());
    assert!(dest_path.exists());
    assert_eq!(
        fs::metadata(&src_path).unwrap().len(),
        fs::metadata(&dest_path).unwrap().len()
    );
}
