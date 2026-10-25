//! Pasting into an archive names entries like a regular paste does and turns a
//! cut into a move by removing the source once it is written.

use flux::app::update::file_ops::paste::{archive_inner_path, paste_name_for, remove_cut_source};
use flux::services::archive::flux_scratch_dir;
use std::os::unix::fs::symlink;

#[test]
fn inner_path_joins_name_below_prefix() {
    assert_eq!(archive_inner_path("", "a.txt"), "a.txt");
    assert_eq!(archive_inner_path("docs", "a.txt"), "docs/a.txt");
    assert_eq!(archive_inner_path("docs/", "a.txt"), "docs/a.txt");
    assert_eq!(archive_inner_path("a/b//", "a.txt"), "a/b/a.txt");
}

#[test]
fn extracted_scratch_file_enters_archive_under_original_name() {
    let tmp = tempfile::Builder::new()
        .suffix(".report.pdf")
        .tempfile_in(flux_scratch_dir())
        .unwrap();
    let raw = tmp
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let name = paste_name_for(tmp.path(), &raw);
    assert_eq!(archive_inner_path("docs", &name), "docs/report.pdf");
}

#[test]
fn cut_removes_file_source() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.txt");
    std::fs::write(&src, "x").unwrap();
    let archive = dir.path().join("out").join("pack.zip");

    remove_cut_source(&src, &archive).unwrap();
    assert!(!src.exists());
}

#[test]
fn cut_removes_folder_source_recursively() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("folder");
    std::fs::create_dir_all(src.join("nested")).unwrap();
    std::fs::write(src.join("nested/b.txt"), "x").unwrap();
    let archive = dir.path().join("pack.zip");

    remove_cut_source(&src, &archive).unwrap();
    assert!(!src.exists());
}

#[test]
fn cut_removes_symlink_without_touching_target() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep.txt"), "x").unwrap();
    let link = dir.path().join("link");
    symlink(&target, &link).unwrap();
    let archive = dir.path().join("pack.zip");

    remove_cut_source(&link, &archive).unwrap();
    assert!(link.symlink_metadata().is_err());
    assert!(target.join("keep.txt").exists());
}

#[test]
fn cut_keeps_source_that_contains_the_archive() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("folder");
    std::fs::create_dir(&src).unwrap();
    let archive = src.join("pack.zip");
    std::fs::write(&archive, "zip").unwrap();

    remove_cut_source(&src, &archive).unwrap();
    assert!(archive.exists());

    remove_cut_source(&archive, &archive).unwrap();
    assert!(archive.exists());
}

#[test]
fn cut_of_missing_source_reports_error() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("pack.zip");
    assert!(remove_cut_source(&dir.path().join("gone"), &archive).is_err());
}
