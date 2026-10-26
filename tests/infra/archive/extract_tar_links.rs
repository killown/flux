//! TAR extraction keeps links as links, skips entries it cannot recreate
//! instead of writing empty files, and never writes through symlinks.

#![cfg(unix)]

use flux::infra::archive::extract_archive_to_dir;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use tar::{Builder, EntryType, Header};

struct Sandbox {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    dest: PathBuf,
}

/// `root/out` is the destination, `root/outside` the place an escape would land.
fn sandbox() -> Sandbox {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let dest = root.join("out");
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::create_dir_all(root.join("outside")).unwrap();
    Sandbox {
        _tmp: tmp,
        root,
        dest,
    }
}

fn file(b: &mut Builder<std::fs::File>, name: &str, data: &[u8]) {
    let mut h = Header::new_gnu();
    h.set_path(name).unwrap();
    h.set_size(data.len() as u64);
    h.set_mode(0o644);
    h.set_cksum();
    b.append(&h, data).unwrap();
}

fn linkish(b: &mut Builder<std::fs::File>, kind: EntryType, name: &str, target: Option<&str>) {
    let mut h = Header::new_gnu();
    h.set_path(name).unwrap();
    h.set_entry_type(kind);
    if let Some(t) = target {
        h.set_link_name(t).unwrap();
    }
    h.set_size(0);
    h.set_mode(0o644);
    h.set_cksum();
    b.append(&h, std::io::empty()).unwrap();
}

fn write_tar(path: &Path, build: impl FnOnce(&mut Builder<std::fs::File>)) {
    let mut b = Builder::new(std::fs::File::create(path).unwrap());
    build(&mut b);
    b.finish().unwrap();
}

#[test]
fn symlinks_and_hard_links_are_recreated_not_emptied() {
    let sb = sandbox();
    let archive = sb.root.join("links.tar");
    write_tar(&archive, |b| {
        file(b, "real.txt", b"data");
        linkish(b, EntryType::Symlink, "soft", Some("real.txt"));
        linkish(b, EntryType::Link, "hard", Some("real.txt"));
    });
    extract_archive_to_dir(&archive, &sb.dest, None).unwrap();

    let soft = sb.dest.join("soft");
    assert!(std::fs::symlink_metadata(&soft)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(std::fs::read_link(&soft).unwrap(), Path::new("real.txt"));
    assert_eq!(std::fs::read_to_string(&soft).unwrap(), "data");

    let hard = sb.dest.join("hard");
    assert_eq!(std::fs::read_to_string(&hard).unwrap(), "data");
    let real_meta = std::fs::metadata(sb.dest.join("real.txt")).unwrap();
    assert_eq!(std::fs::metadata(&hard).unwrap().ino(), real_meta.ino());
    assert_eq!(real_meta.nlink(), 2);
}

#[test]
fn special_and_metadata_entries_do_not_become_empty_files() {
    let sb = sandbox();
    let archive = sb.root.join("special.tar");
    write_tar(&archive, |b| {
        linkish(b, EntryType::XGlobalHeader, "pax_global_header", None);
        linkish(b, EntryType::Fifo, "pipe", None);
        linkish(b, EntryType::Char, "tty", None);
        file(b, "kept.txt", b"kept");
    });
    extract_archive_to_dir(&archive, &sb.dest, None).unwrap();

    for name in ["pax_global_header", "pipe", "tty"] {
        assert!(
            std::fs::symlink_metadata(sb.dest.join(name)).is_err(),
            "`{name}` must not be written as an empty regular file"
        );
    }
    assert_eq!(
        std::fs::read_to_string(sb.dest.join("kept.txt")).unwrap(),
        "kept"
    );
}

#[test]
fn existing_symlinks_in_the_destination_are_not_followed() {
    let sb = sandbox();
    let victim = sb.root.join("victim.txt");
    std::fs::write(&victim, "victim").unwrap();
    std::os::unix::fs::symlink(sb.root.join("outside"), sb.dest.join("escape")).unwrap();
    std::os::unix::fs::symlink(&victim, sb.dest.join("file_link")).unwrap();

    let archive = sb.root.join("follow.tar");
    write_tar(&archive, |b| {
        file(b, "escape/pwn.txt", b"pwned");
        file(b, "file_link", b"new");
        file(b, "safe.txt", b"safe");
    });
    extract_archive_to_dir(&archive, &sb.dest, None).unwrap();

    assert!(!sb.root.join("outside").join("pwn.txt").exists());
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "victim");
    let replaced = std::fs::symlink_metadata(sb.dest.join("file_link")).unwrap();
    assert!(
        replaced.file_type().is_file(),
        "symlink is replaced, not followed"
    );
    assert_eq!(
        std::fs::read_to_string(sb.dest.join("file_link")).unwrap(),
        "new"
    );
    assert_eq!(
        std::fs::read_to_string(sb.dest.join("safe.txt")).unwrap(),
        "safe"
    );
}

#[test]
fn a_symlink_entry_cannot_redirect_later_entries() {
    let sb = sandbox();
    let archive = sb.root.join("redirect.tar");
    write_tar(&archive, |b| {
        linkish(b, EntryType::Symlink, "evil", Some("../outside"));
        file(b, "evil/pwn.txt", b"pwned");
        linkish(b, EntryType::Link, "evil_hard", Some("evil/pwn.txt"));
    });
    extract_archive_to_dir(&archive, &sb.dest, None).unwrap();

    assert!(!sb.root.join("outside").join("pwn.txt").exists());
    assert!(std::fs::symlink_metadata(sb.dest.join("evil"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(std::fs::symlink_metadata(sb.dest.join("evil_hard")).is_err());
}
