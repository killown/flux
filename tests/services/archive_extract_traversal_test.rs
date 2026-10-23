//! Extraction must never write outside the destination directory.

use flux::services::archive::extract_archive_to_dir;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Sandbox with `root/out/extract` as destination and `root` as the escape target.
struct Sandbox {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    dest: PathBuf,
}

fn sandbox() -> Sandbox {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let dest = root.join("out").join("extract");
    Sandbox {
        _tmp: tmp,
        root,
        dest,
    }
}

/// Entry names a hostile archive can carry, relative to `sb.dest`.
fn hostile_names(sb: &Sandbox) -> Vec<String> {
    vec![
        "../../evil_dotdot.txt".to_string(),
        "good/../../../evil_nested.txt".to_string(),
        format!("{}/evil_absolute.txt", sb.root.display()),
    ]
}

fn assert_contained(sb: &Sandbox, format: &str) {
    for name in ["evil_dotdot.txt", "evil_nested.txt", "evil_absolute.txt"] {
        assert!(
            !sb.root.join(name).exists(),
            "{format}: `{name}` escaped the destination"
        );
    }
    assert_eq!(
        std::fs::read_to_string(sb.dest.join("safe.txt")).unwrap(),
        "safe",
        "{format}: ordinary entries must still extract"
    );
}

fn write_zip(path: &Path, names: &[String]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file("safe.txt", opts).unwrap();
    zip.write_all(b"safe").unwrap();
    for name in names {
        zip.start_file(name.as_str(), opts).unwrap();
        zip.write_all(b"pwned").unwrap();
    }
    zip.finish().unwrap();
}

fn write_tar(path: &Path, names: &[String]) {
    let mut builder = tar::Builder::new(std::fs::File::create(path).unwrap());
    let mut entries = vec![("safe.txt".to_string(), b"safe".to_vec())];
    entries.extend(names.iter().map(|n| (n.clone(), b"pwned".to_vec())));
    for (name, data) in entries {
        let mut header = tar::Header::new_gnu();
        // `set_path` refuses `..`, so write the raw name field like an attacker would.
        header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, data.as_slice()).unwrap();
    }
    builder.finish().unwrap();
}

fn write_7z(path: &Path, names: &[String]) {
    let mut writer = sevenz_rust2::ArchiveWriter::create(path).unwrap();
    let mut entries = vec![("safe.txt".to_string(), b"safe".to_vec())];
    entries.extend(names.iter().map(|n| (n.clone(), b"pwned".to_vec())));
    for (name, data) in entries {
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file(&name),
                Some(std::io::Cursor::new(data)),
            )
            .unwrap();
    }
    writer.finish().unwrap();
}

#[test]
fn zip_entries_cannot_escape_the_destination() {
    let sb = sandbox();
    let archive = sb.root.join("hostile.zip");
    write_zip(&archive, &hostile_names(&sb));
    let _ = extract_archive_to_dir(&archive, &sb.dest, None);
    assert_contained(&sb, "zip");
}

#[test]
fn tar_entries_cannot_escape_the_destination() {
    let sb = sandbox();
    let archive = sb.root.join("hostile.tar");
    write_tar(&archive, &hostile_names(&sb));
    let _ = extract_archive_to_dir(&archive, &sb.dest, None);
    assert_contained(&sb, "tar");
}

#[test]
fn sevenz_entries_cannot_escape_the_destination() {
    let sb = sandbox();
    let archive = sb.root.join("hostile.7z");
    write_7z(&archive, &hostile_names(&sb));
    let _ = extract_archive_to_dir(&archive, &sb.dest, None);
    assert_contained(&sb, "7z");
}
