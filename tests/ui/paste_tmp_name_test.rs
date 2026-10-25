//! Pasting or dropping a file must only drop the temp-file prefix from files
//! Flux itself extracted, never from a real file that merely starts with `.tmp`.

use flux::app::update::file_ops::paste::{clean_tmp_basename, paste_name_for};
use flux::services::archive::flux_scratch_dir;
use std::path::Path;

#[test]
fn real_tempfile_names_lose_prefix_and_random_part() {
    assert_eq!(clean_tmp_basename(".tmpAbC123.notes.txt"), "notes.txt");
    assert_eq!(
        clean_tmp_basename(".tmpabcdef.archive.tar.gz"),
        "archive.tar.gz"
    );
    assert_eq!(clean_tmp_basename(".tmp123456.Makefile"), "Makefile");
}

#[test]
fn names_that_only_start_with_tmp_are_untouched() {
    for name in [
        ".tmpnotes.txt",
        ".tmp",
        ".tmp.txt",
        ".tmp.folder.AbC123",
        ".tmpabcde.txt",
        ".tmpabcdefg.txt",
        ".tmpabc-ef.txt",
        ".tmpabcdef.",
        ".tmpabcdef",
        ".tmpé12345.txt",
        "tmpabcdef.txt",
        "notes.txt",
    ] {
        assert_eq!(clean_tmp_basename(name), name, "{name}");
    }
}

#[test]
fn temp_looking_file_outside_scratch_keeps_its_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".tmpabcdef.txt");
    std::fs::write(&path, "mine").unwrap();
    assert_eq!(paste_name_for(&path, ".tmpabcdef.txt"), ".tmpabcdef.txt");
    assert_eq!(
        paste_name_for(Path::new("/home/u/.tmpnotes.txt"), ".tmpnotes.txt"),
        ".tmpnotes.txt"
    );
}

#[test]
fn extracted_scratch_file_gets_its_original_name() {
    let tmp = tempfile::Builder::new()
        .suffix(".report.final.pdf")
        .tempfile_in(flux_scratch_dir())
        .unwrap();
    let name = tmp
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(
        name.starts_with(".tmp"),
        "tempfile default prefix changed: {name}"
    );
    assert_eq!(paste_name_for(tmp.path(), &name), "report.final.pdf");
}
