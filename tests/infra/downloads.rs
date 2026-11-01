//! `infra::downloads`: telling a finished file from one that is still being downloaded.
//!
//! The file watcher and the thumbnail loader both rely on these checks to keep
//! half-written files out of the view, so the cases below follow the scratch
//! files real download tools create.

use flux::infra::downloads::{has_active_download_sibling, is_download_temp};
use std::fs;
use std::path::{Path, PathBuf};

fn touch(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, "x").unwrap();
    path
}

#[test]
fn download_scratch_suffixes_are_recognised() {
    for name in [
        "a.part",
        "a.crdownload",
        "a.download",
        "a.partial",
        "a.tmp",
        "DSC_4207.L8Koxwkl.NEF.part",
    ] {
        assert!(is_download_temp(Path::new(name)), "{name}");
    }
}

#[test]
fn suffix_matching_ignores_case() {
    for name in ["A.PART", "a.CrDownload", "a.Tmp"] {
        assert!(is_download_temp(Path::new(name)), "{name}");
    }
}

#[test]
fn finished_files_are_not_scratch_files() {
    for name in [
        "a.png",
        "a.tar.gz",
        "README",
        "a.part.png",
        "partial",
        "tmp",
    ] {
        assert!(!is_download_temp(Path::new(name)), "{name}");
    }
}

#[test]
fn a_scratch_file_next_to_the_target_means_it_is_still_downloading() {
    let firefox = tempfile::tempdir().unwrap();
    let target = touch(firefox.path(), "DSC_4207.NEF");
    touch(firefox.path(), "DSC_4207.L8Koxwkl.NEF.part");
    assert!(has_active_download_sibling(&target));

    let chromium = tempfile::tempdir().unwrap();
    let target = touch(chromium.path(), "photo.jpg");
    touch(chromium.path(), "photo.jpg.crdownload");
    assert!(has_active_download_sibling(&target));
}

#[test]
fn the_target_does_not_have_to_exist_yet() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "movie.mkv.part");

    assert!(has_active_download_sibling(&dir.path().join("movie.mkv")));
}

#[test]
fn without_a_scratch_file_the_download_is_finished() {
    let dir = tempfile::tempdir().unwrap();
    let target = touch(dir.path(), "a.png");

    assert!(!has_active_download_sibling(&target));
}

#[test]
fn scratch_files_of_other_downloads_do_not_count() {
    let dir = tempfile::tempdir().unwrap();
    let target = touch(dir.path(), "a.png");
    touch(dir.path(), "b.png.part");
    touch(dir.path(), "c.crdownload");

    assert!(!has_active_download_sibling(&target));
}

#[test]
fn ordinary_siblings_do_not_count() {
    let dir = tempfile::tempdir().unwrap();
    let target = touch(dir.path(), "a.png");
    touch(dir.path(), "a.jpg");
    touch(dir.path(), "a.txt");

    assert!(!has_active_download_sibling(&target));
}

#[test]
fn an_unreadable_parent_is_not_a_download() {
    assert!(!has_active_download_sibling(Path::new(
        "/nonexistent-flux-test-dir/a.png"
    )));
    assert!(!has_active_download_sibling(Path::new("/")));
}
