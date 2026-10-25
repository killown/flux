//! Extracting a folder removes the folder prefix exactly once, so nested
//! folders that repeat its name keep their own level instead of merging into
//! the parent.

use flux::services::archive::extract_dir_to_tempdir;
use std::io::Write;
use std::path::Path;

/// Entries of the test archive: `(name, contents)`. `a/a/x` and `a/x` have
/// different contents, so a merge or overwrite is visible.
const ENTRIES: &[(&str, &[u8])] = &[
    ("a/x", b"outer"),
    ("a/a/x", b"inner"),
    ("a/a/a/x", b"innermost"),
    ("b/x", b"other"),
];

fn write_zip(path: &Path) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    for (name, data) in ENTRIES {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

fn write_tar(path: &Path) {
    let mut builder = tar::Builder::new(std::fs::File::create(path).unwrap());
    for (name, data) in ENTRIES {
        let mut header = tar::Header::new_gnu();
        header.set_path(name).unwrap();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, *data).unwrap();
    }
    builder.finish().unwrap();
}

fn write_7z(path: &Path) {
    let mut writer = sevenz_rust2::ArchiveWriter::create(path).unwrap();
    for (name, data) in ENTRIES {
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file(name),
                Some(std::io::Cursor::new(data.to_vec())),
            )
            .unwrap();
    }
    writer.finish().unwrap();
}

fn assert_levels_kept(archive: &Path, format: &str) {
    let out = extract_dir_to_tempdir(archive, "a", None)
        .unwrap_or_else(|e| panic!("{format}: extraction failed: {e:?}"));
    let read = |rel: &str| std::fs::read_to_string(out.join(rel)).ok();

    assert_eq!(read("x").as_deref(), Some("outer"), "{format}: a/x");
    assert_eq!(read("a/x").as_deref(), Some("inner"), "{format}: a/a/x");
    assert_eq!(
        read("a/a/x").as_deref(),
        Some("innermost"),
        "{format}: a/a/a/x"
    );
    assert!(!out.join("b").exists(), "{format}: sibling folder leaked");
}

#[test]
fn zip_extract_dir_strips_the_prefix_once() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("nested.zip");
    write_zip(&archive);
    assert_levels_kept(&archive, "zip");
}

#[test]
fn tar_extract_dir_strips_the_prefix_once() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("nested.tar");
    write_tar(&archive);
    assert_levels_kept(&archive, "tar");
}

#[test]
fn sevenz_extract_dir_strips_the_prefix_once() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("nested.7z");
    write_7z(&archive);
    assert_levels_kept(&archive, "7z");
}
