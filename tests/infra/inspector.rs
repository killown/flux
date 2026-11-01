//! `scan_directory_native`: the totals behind the directory inspector dialog.
//!
//! Each test builds its own tree under a temp dir, so none of it depends on
//! the machine the suite runs on.

use flux::infra::inspector::scan_directory_native;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

fn tree() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    (dir, root)
}

fn write(root: &Path, rel: &str, len: usize) -> PathBuf {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, vec![b'x'; len]).unwrap();
    path
}

#[test]
fn an_empty_directory_reports_nothing() {
    let (_guard, root) = tree();

    let stats = scan_directory_native(&root);

    assert_eq!(stats.total_files, 0);
    assert_eq!(stats.total_dirs, 0);
    assert_eq!(stats.total_bytes, 0);
    assert!(stats.top_exts.is_empty());
    assert!(stats.largest_files.is_empty());
}

#[test]
fn a_missing_directory_reports_nothing() {
    let (_guard, root) = tree();

    let stats = scan_directory_native(&root.join("does-not-exist"));

    assert_eq!(stats.total_files, 0);
    assert_eq!(stats.total_dirs, 0);
    assert_eq!(stats.total_bytes, 0);
}

#[test]
fn counts_files_directories_and_bytes_recursively() {
    let (_guard, root) = tree();
    write(&root, "a.txt", 10);
    write(&root, "sub/b.txt", 20);
    write(&root, "sub/deeper/c.bin", 30);
    fs::create_dir(root.join("empty")).unwrap();

    let stats = scan_directory_native(&root);

    assert_eq!(stats.total_files, 3);
    assert_eq!(stats.total_bytes, 60);
    // `sub`, `sub/deeper` and `empty`, the scanned directory is not counted.
    assert_eq!(stats.total_dirs, 3);
}

#[test]
fn hidden_files_and_directories_are_counted() {
    let (_guard, root) = tree();
    write(&root, ".hidden", 5);
    write(&root, ".cache/blob", 7);

    let stats = scan_directory_native(&root);

    assert_eq!(stats.total_files, 2);
    assert_eq!(stats.total_dirs, 1);
    assert_eq!(stats.total_bytes, 12);
}

#[test]
fn ignore_files_do_not_hide_anything() {
    let (_guard, root) = tree();
    fs::create_dir(root.join(".git")).unwrap();
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    fs::write(root.join(".ignore"), "*.log\n").unwrap();
    write(&root, "a.log", 100);
    write(&root, "b.txt", 50);

    let stats = scan_directory_native(&root);

    assert_eq!(stats.total_files, 4);
    assert_eq!(stats.total_dirs, 1);
    assert_eq!(stats.total_bytes, 6 + 6 + 100 + 50);
}

#[test]
fn symlinks_are_not_followed_or_double_counted() {
    let (_guard, root) = tree();
    write(&root, "real/data", 40);
    symlink("real", root.join("link-to-dir")).unwrap();
    symlink("real/data", root.join("link-to-file")).unwrap();

    let stats = scan_directory_native(&root);

    assert_eq!(stats.total_files, 1);
    assert_eq!(stats.total_dirs, 1);
    assert_eq!(stats.total_bytes, 40);
}

#[test]
fn extensions_are_grouped_case_insensitively_and_ranked_by_size() {
    let (_guard, root) = tree();
    write(&root, "a.TXT", 10);
    write(&root, "b.txt", 20);
    write(&root, "c.Txt", 30);
    write(&root, "README", 5);
    write(&root, "archive.tar.gz", 100);

    let stats = scan_directory_native(&root);

    assert_eq!(
        stats.top_exts,
        vec![
            ("gz".to_string(), 1, 100),
            ("txt".to_string(), 3, 60),
            ("(none)".to_string(), 1, 5),
        ]
    );
}

#[test]
fn a_dotfile_has_no_extension() {
    let (_guard, root) = tree();
    write(&root, ".bashrc", 8);

    let stats = scan_directory_native(&root);

    assert_eq!(stats.top_exts, vec![("(none)".to_string(), 1, 8)]);
}

#[test]
fn only_the_ten_biggest_extensions_are_kept() {
    let (_guard, root) = tree();
    for i in 1..=12usize {
        write(&root, &format!("file.ext{i:02}"), i * 10);
    }

    let stats = scan_directory_native(&root);

    assert_eq!(stats.total_files, 12, "totals still cover every file");
    assert_eq!(stats.top_exts.len(), 10);
    assert_eq!(stats.top_exts[0], ("ext12".to_string(), 1, 120));
    assert_eq!(stats.top_exts[9], ("ext03".to_string(), 1, 30));
    assert!(stats
        .top_exts
        .iter()
        .all(|(ext, _, _)| ext != "ext01" && ext != "ext02"));
}

#[test]
fn only_the_hundred_largest_files_are_kept_largest_first() {
    let (_guard, root) = tree();
    for size in 1..=105usize {
        write(&root, &format!("d{}/f{size}.dat", size % 5), size);
    }

    let stats = scan_directory_native(&root);

    assert_eq!(stats.total_files, 105);
    assert_eq!(stats.largest_files.len(), 100);
    let sizes: Vec<u64> = stats.largest_files.iter().map(|(_, size)| *size).collect();
    let expected: Vec<u64> = (6..=105u64).rev().collect();
    assert_eq!(sizes, expected);
    assert_eq!(stats.largest_files[0].0, root.join("d0/f105.dat"));
}

#[test]
fn totals_stay_exact_when_many_workers_share_the_walk() {
    let (_guard, root) = tree();
    for dir in 0..8 {
        for file in 0..40 {
            write(&root, &format!("dir{dir}/file{file}.dat"), 3);
        }
    }

    let stats = scan_directory_native(&root);

    assert_eq!(stats.total_files, 320);
    assert_eq!(stats.total_dirs, 8);
    assert_eq!(stats.total_bytes, 960);
    assert_eq!(stats.top_exts, vec![("dat".to_string(), 320, 960)]);
}
