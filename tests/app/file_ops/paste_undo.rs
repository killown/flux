//! Regression tests for the guards that keep "undo copy" from deleting data
//! the user already had.

use std::fs::{self, File};
use std::os::unix::fs::symlink;

use flux::app::update::file_ops::paste::is_same_entry;
use tempfile::tempdir;

#[test]
fn same_path_is_same_entry() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("a.txt");
    File::create(&f).unwrap();

    assert!(is_same_entry(&f, &f));
}

#[test]
fn symlinked_folder_alias_is_same_entry() {
    let dir = tempdir().unwrap();
    let real = dir.path().join("real");
    let alias = dir.path().join("alias");
    fs::create_dir(&real).unwrap();
    symlink(&real, &alias).unwrap();
    File::create(real.join("a.txt")).unwrap();

    assert!(is_same_entry(&real.join("a.txt"), &alias.join("a.txt")));
}

#[test]
fn same_name_in_different_folders_is_not_same_entry() {
    let dir = tempdir().unwrap();
    let d1 = dir.path().join("d1");
    let d2 = dir.path().join("d2");
    fs::create_dir(&d1).unwrap();
    fs::create_dir(&d2).unwrap();
    File::create(d1.join("a.txt")).unwrap();
    File::create(d2.join("a.txt")).unwrap();

    assert!(!is_same_entry(&d1.join("a.txt"), &d2.join("a.txt")));
}

#[test]
fn different_names_in_same_folder_are_not_same_entry() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("a.txt")).unwrap();
    File::create(dir.path().join("b.txt")).unwrap();

    assert!(!is_same_entry(
        &dir.path().join("a.txt"),
        &dir.path().join("b.txt")
    ));
}

#[test]
fn symlink_pointing_at_source_is_a_different_entry() {
    let dir = tempdir().unwrap();
    let d1 = dir.path().join("d1");
    let d2 = dir.path().join("d2");
    fs::create_dir(&d1).unwrap();
    fs::create_dir(&d2).unwrap();
    let src = d1.join("a.txt");
    File::create(&src).unwrap();
    symlink(&src, d2.join("a.txt")).unwrap();

    assert!(!is_same_entry(&src, &d2.join("a.txt")));
}
