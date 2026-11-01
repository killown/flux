//! `plan_deletion`: which paths the UI may delete and which it must refuse.
//!
//! The protected-target guard is the last line of defence against deleting
//! `/`, the home directory or a system directory from the file manager, so it
//! is exercised through the public entry point rather than a copy of its rules.
//! Nothing here deletes anything: `plan_deletion` only classifies.

use flux::infra::archive::build_archive_uri;
use flux::infra::trash::{plan_deletion, DeletePlan};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

fn plan_for(path: impl Into<PathBuf>) -> DeletePlan {
    plan_deletion(vec![path.into()], None)
}

fn assert_nothing_scheduled(plan: &DeletePlan) {
    assert!(plan.local.is_empty(), "local: {:?}", plan.local);
    assert!(plan.network.is_empty(), "network: {:?}", plan.network);
    assert!(plan.archives.is_empty(), "archives: {:?}", plan.archives);
}

#[test]
fn system_directories_are_protected() {
    for dir in [
        "/",
        "/boot",
        "/dev",
        "/etc",
        "/lost+found",
        "/media",
        "/mnt",
        "/proc",
        "/root",
        "/run",
        "/run/media",
        "/sys",
        "/tmp",
        "/usr",
        "/var",
    ] {
        let plan = plan_for(dir);
        assert_eq!(plan.protected, vec![PathBuf::from(dir)], "{dir}");
        assert_nothing_scheduled(&plan);
    }
}

#[test]
fn protected_paths_are_reported_in_their_resolved_form() {
    // `..` segments and a trailing slash must not be a way around the guard.
    for spelling in ["/etc/../etc", "/etc/", "/usr/../etc/"] {
        let plan = plan_for(spelling);
        assert_eq!(plan.protected, vec![PathBuf::from("/etc")], "{spelling}");
        assert_nothing_scheduled(&plan);
    }
}

#[test]
fn home_is_protected_but_things_inside_it_are_not() {
    let Some(home) = dirs::home_dir() else {
        return;
    };

    let plan = plan_for(&home);
    assert_eq!(plan.protected.len(), 1, "home must be refused");
    assert_nothing_scheduled(&plan);

    let inside = home.join(".flux-plan-deletion-test-does-not-exist");
    let plan = plan_for(&inside);
    assert!(plan.protected.is_empty());
    assert_eq!(plan.local.len(), 1);
    assert_eq!(plan.local[0].1, inside);
}

#[test]
fn files_inside_a_protected_directory_are_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("note.txt");
    std::fs::write(&file, "x").unwrap();

    let plan = plan_for(&file);
    assert!(plan.protected.is_empty());
    assert_eq!(plan.local.len(), 1);
    assert_eq!(plan.local[0].1, file);
    assert!(
        plan.local[0].0.starts_with("file://"),
        "{}",
        plan.local[0].0
    );
}

#[test]
fn a_symlink_to_a_protected_directory_is_deleted_as_a_link() {
    let dir = tempfile::tempdir().unwrap();
    for (name, target) in [("etc-link", "/etc"), ("root-link", "/")] {
        let link = dir.path().join(name);
        symlink(target, &link).unwrap();

        let plan = plan_for(&link);
        assert!(plan.protected.is_empty(), "{name} -> {target}");
        assert_eq!(plan.local.len(), 1, "{name} -> {target}");
        assert_eq!(plan.local[0].1, link);
    }
}

#[test]
fn a_selection_is_split_without_reordering_the_deletable_entries() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::write(&first, "1").unwrap();
    std::fs::write(&second, "2").unwrap();

    let plan = plan_deletion(
        vec![
            first.clone(),
            PathBuf::from("/etc"),
            second.clone(),
            PathBuf::from("/"),
        ],
        None,
    );

    assert_eq!(
        plan.protected,
        vec![PathBuf::from("/etc"), PathBuf::from("/")]
    );
    let local: Vec<&Path> = plan.local.iter().map(|(_, p)| p.as_path()).collect();
    assert_eq!(local, [first.as_path(), second.as_path()]);
    assert!(plan.network.is_empty());
    assert!(plan.archives.is_empty());
}

#[test]
fn an_empty_selection_falls_back_to_the_active_item() {
    let dir = tempfile::tempdir().unwrap();
    let active = dir.path().join("active");
    let selected = dir.path().join("selected");

    let plan = plan_deletion(Vec::new(), Some(active.clone()));
    assert_eq!(plan.local.len(), 1);
    assert_eq!(plan.local[0].1, active);

    let plan = plan_deletion(vec![selected.clone()], Some(active));
    assert_eq!(plan.local.len(), 1);
    assert_eq!(plan.local[0].1, selected);

    let plan = plan_deletion(Vec::new(), None);
    assert!(plan.protected.is_empty());
    assert_nothing_scheduled(&plan);
}

#[test]
fn the_root_of_a_virtual_filesystem_is_protected() {
    let archive_root = build_archive_uri(Path::new("/data/photos.zip"), "");
    for root in ["network:///", "trash:///", "smb://server/", "sftp://server"] {
        let plan = plan_for(root);
        assert_eq!(plan.protected.len(), 1, "{root}");
        assert_nothing_scheduled(&plan);
    }

    let plan = plan_for(archive_root);
    assert_eq!(plan.protected.len(), 1, "archive root");
    assert_nothing_scheduled(&plan);
}

#[test]
fn network_entries_are_deleted_over_the_network_not_trashed() {
    let plan = plan_for("smb://server/share/file.txt");

    assert!(plan.protected.is_empty());
    assert!(plan.local.is_empty());
    assert!(plan.archives.is_empty());
    assert_eq!(plan.network.len(), 1);
    assert!(plan.network[0].starts_with("smb://"), "{}", plan.network[0]);
    assert!(
        plan.network[0].ends_with("/share/file.txt"),
        "{}",
        plan.network[0]
    );
}

#[test]
fn entries_inside_an_archive_are_never_scheduled_for_disk_deletion() {
    let uri = build_archive_uri(Path::new("/data/photos.zip"), "2024/img.png");

    let plan = plan_for(uri);

    assert!(plan.protected.is_empty());
    assert_eq!(
        plan.archives,
        vec![(
            PathBuf::from("/data/photos.zip"),
            "2024/img.png".to_string()
        )]
    );
    assert!(plan.local.is_empty());
    assert!(plan.network.is_empty());
}
