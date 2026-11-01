//! `delete_recursive`, `delete_network_uri` and `trash_local_batch`: the blocking
//! half of deleting, which runs without a `FluxApp`.
//!
//! `delete_network_uri` is what network entries go through because those
//! filesystems have no trash. It works on any GIO URI, so the tests run it on
//! `file://` URIs inside a temp dir. The trash itself is never written to:
//! `trash_local_batch` is only given files that do not exist.

use flux::infra::trash::{delete_network_uri, delete_recursive, trash_local_batch};
use gtk::gio;
use gtk::gio::prelude::FileExt;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

fn uri(path: &Path) -> String {
    gio::File::for_path(path).uri().to_string()
}

#[test]
fn a_single_file_is_removed() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("note.txt");
    fs::write(&file, "x").unwrap();

    delete_network_uri(&uri(&file)).unwrap();

    assert!(!file.exists());
    assert!(dir.path().exists());
}

#[test]
fn a_non_empty_tree_is_removed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir_all(root.join("a/b")).unwrap();
    fs::create_dir(root.join("empty")).unwrap();
    fs::write(root.join("a/b/c.txt"), "c").unwrap();
    fs::write(root.join("a/d.txt"), "d").unwrap();
    fs::write(root.join("top.txt"), "t").unwrap();

    delete_network_uri(&uri(&root)).unwrap();

    assert!(!root.exists());
    assert!(dir.path().exists());
}

#[test]
fn a_missing_path_is_an_error_and_touches_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let bystander = dir.path().join("bystander.txt");
    fs::write(&bystander, "x").unwrap();

    let err = delete_network_uri(&uri(&dir.path().join("missing"))).unwrap_err();

    assert!(err.matches(gio::IOErrorEnum::NotFound), "{err}");
    assert!(bystander.exists());
}

#[test]
fn symlinks_are_removed_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep.txt"), "keep").unwrap();
    let outside_file = dir.path().join("outside.txt");
    fs::write(&outside_file, "keep").unwrap();

    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    symlink(&outside, root.join("dir-link")).unwrap();
    symlink(&outside_file, root.join("file-link")).unwrap();

    delete_network_uri(&uri(&root)).unwrap();

    assert!(!root.exists());
    assert!(outside.join("keep.txt").exists());
    assert!(outside_file.exists());
}

#[test]
fn a_symlink_to_a_directory_is_unlinked_without_entering_it() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep.txt"), "keep").unwrap();
    let link = dir.path().join("link");
    symlink(&target, &link).unwrap();

    delete_recursive(&gio::File::for_path(&link)).unwrap();

    assert!(fs::symlink_metadata(&link).is_err());
    assert!(target.join("keep.txt").exists());
}

#[test]
fn an_empty_batch_trashes_nothing() {
    let (trashed, failed) = trash_local_batch(Vec::new());

    assert!(trashed.is_empty());
    assert!(failed.is_empty());
}

#[test]
fn files_that_cannot_be_trashed_are_reported_by_path_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("missing-first");
    let second = dir.path().join("missing-second");
    let targets: Vec<(String, PathBuf)> = [&first, &second]
        .into_iter()
        .map(|p| (uri(p), p.clone()))
        .collect();

    let (trashed, failed) = trash_local_batch(targets);

    assert!(trashed.is_empty());
    let paths: Vec<&Path> = failed.iter().map(|f| f.path.as_path()).collect();
    assert_eq!(paths, [first.as_path(), second.as_path()]);
}
