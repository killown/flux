use flux::services::archive::{is_browsable_archive, is_supported_archive, list_archive_entries};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;
use zip::write::FileOptions;
use zip::ZipWriter;

#[test]
fn test_archive_supported_formats_exhaustive() {
    let supported = [
        "test.zip",
        "test.7z",
        "test.rar",
        "test.iso",
        "test.tar",
        "test.tar.gz",
        "test.tgz",
        "test.tar.bz2",
        "test.tbz2",
        "test.tar.xz",
        "test.txz",
        "test.tar.zst",
        "test.tzst",
        "test.tar.lz4",
        "test.gz",
        "test.bz2",
        "test.xz",
        "test.zst",
        "test.zstd",
        "test.lz4",
    ];

    for file in supported {
        assert!(
            is_supported_archive(Path::new(file)),
            "Format {} must be identified as supported archive",
            file
        );
        assert!(
            is_browsable_archive(Path::new(file)),
            "Format {} must be identified as browsable archive",
            file
        );
    }

    let unsupported = ["test.txt", "test.png", "test.rs", "test.tar.unknown"];
    for file in unsupported {
        assert!(!is_supported_archive(Path::new(file)));
        assert!(!is_browsable_archive(Path::new(file)));
    }
}

#[test]
fn test_synthesized_directory_nodes_deep_nesting() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("deep.zip");
    let file = File::create(&zip_path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default();

    zip.start_file("a/b/c/d/deep_file.txt", options).unwrap();
    zip.write_all(b"payload").unwrap();
    zip.finish().unwrap();

    let root_entries = list_archive_entries(&zip_path, "", None).unwrap();
    assert_eq!(root_entries.len(), 1);
    assert_eq!(root_entries[0].name, "a");
    assert!(root_entries[0].is_dir);
    assert_eq!(root_entries[0].size, 0);

    let c_entries = list_archive_entries(&zip_path, "a/b/c", None).unwrap();
    assert_eq!(c_entries.len(), 1);
    assert_eq!(c_entries[0].name, "d");
    assert!(c_entries[0].is_dir);

    let d_entries = list_archive_entries(&zip_path, "a/b/c/d", None).unwrap();
    assert_eq!(d_entries.len(), 1);
    assert_eq!(d_entries[0].name, "deep_file.txt");
    assert!(!d_entries[0].is_dir);
    assert_eq!(d_entries[0].size, 7);
}
