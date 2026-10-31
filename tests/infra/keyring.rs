use flux::domain::model::PendingSaves;
use flux::infra::keyring::{
    forget_archive_password, lookup_archive_password, save_archive_password, ArchiveKey,
    APPLICATION_ID, KIND_ARCHIVE_PASSWORD,
};
use std::fs::OpenOptions;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};
use tempfile::tempdir;

fn block_on<F: Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(f)
}

fn set_mtime(path: &Path, secs: u64, nanos: u32) {
    OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for mtime")
        .set_modified(UNIX_EPOCH + Duration::new(secs, nanos))
        .expect("set mtime");
}

#[test]
fn attributes_identify_application_kind_path_and_mtime() {
    let attrs = ArchiveKey::new("/data/vault.7z", 1_700_000_000).attributes();

    assert_eq!(attrs["application"], APPLICATION_ID);
    assert_eq!(attrs["kind"], KIND_ARCHIVE_PASSWORD);
    assert_eq!(attrs["archive-path"], "/data/vault.7z");
    assert_eq!(attrs["archive-mtime"], "1700000000");
    assert_eq!(attrs.len(), 4, "no stray attributes: {attrs:?}");
}

#[test]
fn replaced_archive_gets_a_different_key() {
    let old = ArchiveKey::new("/data/vault.7z", 1_700_000_000).attributes();
    let new = ArchiveKey::new("/data/vault.7z", 1_700_000_001).attributes();
    assert_ne!(
        old, new,
        "same name, new mtime must not match the old secret"
    );
}

#[test]
fn different_archives_never_share_a_key() {
    let a = ArchiveKey::new("/data/a.zip", 42).attributes();
    let b = ArchiveKey::new("/data/b.zip", 42).attributes();
    assert_ne!(a, b);
}

#[test]
fn identical_path_and_mtime_give_an_identical_key() {
    let a = ArchiveKey::new("/data/a.zip", 42);
    let b = ArchiveKey::new("/data/a.zip", 42);
    assert_eq!(a, b);
    assert_eq!(a.attributes(), b.attributes());
}

#[test]
fn label_names_the_archive_file() {
    let label = ArchiveKey::new("/data/deep/dir/vault.7z", 1).label();
    assert!(label.contains("vault.7z"), "label was {label:?}");
    assert!(
        !label.contains("/data/deep"),
        "label must not leak the directory"
    );
}

#[test]
fn for_path_uses_the_canonical_path_and_mtime_seconds() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.zip");
    std::fs::write(&file, b"x").unwrap();
    set_mtime(&file, 1_700_000_000, 0);

    let key = block_on(ArchiveKey::for_path(&file)).unwrap();

    assert_eq!(
        key.path,
        std::fs::canonicalize(&file).unwrap().to_string_lossy()
    );
    assert_eq!(key.mtime_secs, 1_700_000_000);
}

#[test]
fn for_path_ignores_the_sub_second_part_of_the_mtime() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.zip");
    std::fs::write(&file, b"x").unwrap();
    set_mtime(&file, 1_700_000_000, 900_000_000);

    let key = block_on(ArchiveKey::for_path(&file)).unwrap();
    assert_eq!(key.mtime_secs, 1_700_000_000);
}

#[test]
fn a_symlink_and_its_target_resolve_to_the_same_key() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("real.zip");
    let link = dir.path().join("link.zip");
    std::fs::write(&file, b"x").unwrap();
    std::os::unix::fs::symlink(&file, &link).unwrap();

    let via_file = block_on(ArchiveKey::for_path(&file)).unwrap();
    let via_link = block_on(ArchiveKey::for_path(&link)).unwrap();
    assert_eq!(via_file, via_link);
}

#[test]
fn touching_the_archive_changes_the_key() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.zip");
    std::fs::write(&file, b"x").unwrap();

    set_mtime(&file, 1_700_000_000, 0);
    let before = block_on(ArchiveKey::for_path(&file)).unwrap();
    set_mtime(&file, 1_700_000_500, 0);
    let after = block_on(ArchiveKey::for_path(&file)).unwrap();

    assert_ne!(before.attributes(), after.attributes());
}

#[test]
fn for_path_fails_for_a_missing_file() {
    let dir = tempdir().unwrap();
    assert!(block_on(ArchiveKey::for_path(&dir.path().join("gone.zip"))).is_err());
}

#[test]
fn operations_on_a_missing_archive_are_non_fatal() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("gone.zip");

    assert_eq!(block_on(lookup_archive_password(&missing)), None);
    assert!(!block_on(save_archive_password(&missing, "hunter2")));
    assert!(!block_on(forget_archive_password(&missing)));
}

// ── path_attributes: sweeping entries saved under older mtimes ──────────────

#[test]
fn path_attributes_drop_only_the_mtime() {
    let key = ArchiveKey::new("/data/vault.7z", 1_700_000_000);
    let full = key.attributes();
    let path_only = key.path_attributes();

    assert!(!path_only.contains_key("archive-mtime"));
    assert_eq!(path_only.len(), full.len() - 1);
    for (k, v) in &path_only {
        assert_eq!(full[k], *v, "attribute {k} must be unchanged");
    }
}

#[test]
fn path_attributes_are_identical_across_mtimes_but_not_across_paths() {
    let old = ArchiveKey::new("/data/vault.7z", 1).path_attributes();
    let new = ArchiveKey::new("/data/vault.7z", 2).path_attributes();
    let other = ArchiveKey::new("/data/other.7z", 1).path_attributes();

    assert_eq!(old, new, "a sweep must match every earlier mtime");
    assert_ne!(old, other, "a sweep must never reach another archive");
}

// ── PendingSaves: a wrong password must never be persisted ──────────────────

#[test]
fn queued_save_is_released_only_for_the_accepted_password() {
    let mut pending = PendingSaves::default();
    pending.queue(PathBuf::from("/a.zip"), "right".into());

    assert_eq!(
        pending.confirm(Path::new("/a.zip"), Some("right")),
        Some("right".to_string())
    );
    assert_eq!(
        pending.confirm(Path::new("/a.zip"), Some("right")),
        None,
        "confirm consumes the request"
    );
}

#[test]
fn a_different_accepted_password_does_not_release_the_save() {
    let mut pending = PendingSaves::default();
    pending.queue(PathBuf::from("/a.zip"), "typo".into());

    assert_eq!(pending.confirm(Path::new("/a.zip"), Some("other")), None);
    assert_eq!(
        pending.confirm(Path::new("/a.zip"), Some("typo")),
        None,
        "the request is still consumed"
    );
}

#[test]
fn an_unlock_without_a_password_does_not_release_the_save() {
    let mut pending = PendingSaves::default();
    pending.queue(PathBuf::from("/a.zip"), "pw".into());
    assert_eq!(pending.confirm(Path::new("/a.zip"), None), None);
}

#[test]
fn discard_drops_the_request_so_a_later_success_saves_nothing() {
    let mut pending = PendingSaves::default();
    pending.queue(PathBuf::from("/a.zip"), "typo".into());

    pending.discard(Path::new("/a.zip"));

    assert_eq!(pending.confirm(Path::new("/a.zip"), Some("typo")), None);
}

#[test]
fn requests_are_tracked_per_archive() {
    let mut pending = PendingSaves::default();
    pending.queue(PathBuf::from("/a.zip"), "pa".into());
    pending.queue(PathBuf::from("/b.zip"), "pb".into());

    assert_eq!(
        pending.confirm(Path::new("/a.zip"), Some("pa")),
        Some("pa".into())
    );
    assert_eq!(
        pending.confirm(Path::new("/b.zip"), Some("pb")),
        Some("pb".into())
    );
}

#[test]
fn confirming_an_unknown_archive_is_a_no_op() {
    let mut pending = PendingSaves::default();
    assert_eq!(pending.confirm(Path::new("/none.zip"), Some("x")), None);
}

#[test]
fn debug_output_never_contains_a_password() {
    let mut pending = PendingSaves::default();
    pending.queue(PathBuf::from("/a.zip"), "s3cret-value".into());
    assert!(!format!("{pending:?}").contains("s3cret-value"));
}
