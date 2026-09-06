use flux::services::archive::{
    build_archive_uri, decode_archive_host, encode_archive_host, entries_to_load_contexts,
    extract_entry_to_tempfile, is_browsable_archive, list_archive_entries, parse_archive_uri,
    ArchiveEntry,
};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use zip::write::FileOptions;
use zip::ZipWriter;

fn create_test_zip() -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("test.zip");
    let file = File::create(&zip_path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default();

    zip.start_file("file1.txt", options).unwrap();
    zip.write_all(b"Hello").unwrap();

    zip.start_file("dir/file2.txt", options).unwrap();
    zip.write_all(b"World").unwrap();

    zip.start_file("dir/sub/file3.txt", options).unwrap();
    zip.write_all(b"Foo").unwrap();

    zip.finish().unwrap();
    (dir, zip_path)
}

fn create_test_tar() -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let tar_path = dir.path().join("test.tar");
    let mut builder = tar::Builder::new(File::create(&tar_path).unwrap());
    let mut header = tar::Header::new_gnu();
    header.set_path("file1.txt").unwrap();
    header.set_size(5);
    header.set_cksum();
    builder.append(&header, &b"Hello"[..]).unwrap();

    let mut header = tar::Header::new_gnu();
    header.set_path("dir/file2.txt").unwrap();
    header.set_size(5);
    header.set_cksum();
    builder.append(&header, &b"World"[..]).unwrap();

    let mut header = tar::Header::new_gnu();
    header.set_path("dir/sub/file3.txt").unwrap();
    header.set_size(3);
    header.set_cksum();
    builder.append(&header, &b"Foo"[..]).unwrap();

    builder.into_inner().unwrap();
    (dir, tar_path)
}

fn create_compressed_tar(
    ext: &str,
    compress: impl Fn(&[u8]) -> Vec<u8>,
) -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let file_name = format!("test.tar.{}", ext);
    let path = dir.path().join(file_name);

    let mut tar_data = Vec::new();
    let mut builder = tar::Builder::new(&mut tar_data);
    let mut header = tar::Header::new_gnu();
    header.set_path("file1.txt").unwrap();
    header.set_size(5);
    header.set_cksum();
    builder.append(&header, &b"Hello"[..]).unwrap();

    let mut header = tar::Header::new_gnu();
    header.set_path("dir/file2.txt").unwrap();
    header.set_size(5);
    header.set_cksum();
    builder.append(&header, &b"World"[..]).unwrap();

    builder.into_inner().unwrap();

    let compressed = compress(&tar_data);
    fs::write(&path, &compressed).unwrap();
    (dir, path)
}

#[test]
fn test_encode_decode_host() {
    let path = Path::new("/home/user/archive.zip");
    let encoded = encode_archive_host(path);
    assert_eq!(encoded, "%2Fhome%2Fuser%2Farchive.zip");
    let decoded = decode_archive_host(&encoded);
    assert_eq!(decoded, path);
}

#[test]
fn test_encode_decode_host_with_special_chars() {
    let path = Path::new("/path/with%20space/archive.zip");
    let encoded = encode_archive_host(path);
    assert_eq!(encoded, "%2Fpath%2Fwith%2520space%2Farchive.zip");
    let decoded = decode_archive_host(&encoded);
    assert_eq!(decoded, path);
}

#[test]
fn test_parse_archive_uri() {
    let uri = "/archive://%2Fhome%2Fuser%2Farchive.zip/dir/file.txt";
    let (archive_path, inner) = parse_archive_uri(uri).unwrap();
    assert_eq!(archive_path, Path::new("/home/user/archive.zip"));
    assert_eq!(inner, "dir/file.txt");
}

#[test]
fn test_parse_archive_uri_root() {
    let uri = "/archive://%2Fhome%2Fuser%2Farchive.zip/";
    let (archive_path, inner) = parse_archive_uri(uri).unwrap();
    assert_eq!(archive_path, Path::new("/home/user/archive.zip"));
    assert_eq!(inner, "");
}

#[test]
fn test_parse_archive_uri_no_inner() {
    let uri = "/archive://%2Fhome%2Fuser%2Farchive.zip";
    let (archive_path, inner) = parse_archive_uri(uri).unwrap();
    assert_eq!(archive_path, Path::new("/home/user/archive.zip"));
    assert_eq!(inner, "");
}

#[test]
fn test_parse_archive_uri_invalid() {
    assert!(parse_archive_uri("archive://").is_none());
    assert!(parse_archive_uri("/archive://").is_none());
    assert!(parse_archive_uri("/archive:///").is_none());
}

#[test]
fn test_build_archive_uri() {
    let archive = Path::new("/home/user/archive.zip");
    let uri = build_archive_uri(archive, "dir/file.txt");
    assert_eq!(
        uri.to_string_lossy(),
        "/archive://%2Fhome%2Fuser%2Farchive.zip/dir/file.txt"
    );
}

#[test]
fn test_build_archive_uri_root() {
    let archive = Path::new("/home/user/archive.zip");
    let uri = build_archive_uri(archive, "");
    assert_eq!(
        uri.to_string_lossy(),
        "/archive://%2Fhome%2Fuser%2Farchive.zip/"
    );
}

#[test]
fn test_is_browsable_archive() {
    let cases = [
        ("archive.zip", true),
        ("archive.tar", true),
        ("archive.tar.gz", true),
        ("archive.tgz", true),
        ("archive.tar.bz2", true),
        ("archive.tbz2", true),
        ("archive.tar.xz", true),
        ("archive.txz", true),
        ("archive.7z", true),
        ("archive.rar", true),
        ("file.txt", false),
        ("archive.ZIP", true),
        ("archive.TAR.GZ", true),
    ];
    for (name, expected) in cases {
        let path = Path::new(name);
        assert_eq!(is_browsable_archive(path), expected, "failed for {}", name);
    }
}

#[test]
fn test_list_zip_root() {
    let (_dir, zip_path) = create_test_zip();
    let entries = list_archive_entries(&zip_path, "", None).unwrap();
    assert_eq!(entries.len(), 2);
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"file1.txt"));
    assert!(names.contains(&"dir"));
    let dir_entry = entries.iter().find(|e| e.name == "dir").unwrap();
    assert!(dir_entry.is_dir);
    assert_eq!(dir_entry.size, 0);
    assert_eq!(dir_entry.inner_path, "dir");
}

#[test]
fn test_list_zip_subdir() {
    let (_dir, zip_path) = create_test_zip();
    let entries = list_archive_entries(&zip_path, "dir", None).unwrap();
    assert_eq!(entries.len(), 2);
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"file2.txt"));
    assert!(names.contains(&"sub"));
    let sub_entry = entries.iter().find(|e| e.name == "sub").unwrap();
    assert!(sub_entry.is_dir);
    assert_eq!(sub_entry.inner_path, "dir/sub");
}

#[test]
fn test_list_zip_nested_subdir() {
    let (_dir, zip_path) = create_test_zip();
    let entries = list_archive_entries(&zip_path, "dir/sub", None).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "file3.txt");
    assert!(!entries[0].is_dir);
    assert_eq!(entries[0].size, 3);
    assert_eq!(entries[0].inner_path, "dir/sub/file3.txt");
}

#[test]
fn test_list_tar_root() {
    let (_dir, tar_path) = create_test_tar();
    let entries = list_archive_entries(&tar_path, "", None).unwrap();
    assert_eq!(entries.len(), 2);
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"file1.txt"));
    assert!(names.contains(&"dir"));
    let dir_entry = entries.iter().find(|e| e.name == "dir").unwrap();
    assert!(dir_entry.is_dir);
    assert_eq!(dir_entry.inner_path, "dir");
}

#[test]
fn test_list_tar_gz() {
    let (_dir, path) = create_compressed_tar("gz", |data| {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    });
    let entries = list_archive_entries(&path, "", None).unwrap();
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_list_tar_bz2() {
    let (_dir, path) = create_compressed_tar("bz2", |data| {
        use bzip2::write::BzEncoder;
        use bzip2::Compression;
        let mut encoder = BzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    });
    let entries = list_archive_entries(&path, "", None).unwrap();
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_list_tar_xz() {
    let (_dir, path) = create_compressed_tar("xz", |data| {
        use xz2::write::XzEncoder;
        let mut encoder = XzEncoder::new(Vec::new(), 6);
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    });
    let entries = list_archive_entries(&path, "", None).unwrap();
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_list_archive_empty_zip() {
    let dir = tempdir().unwrap();
    let zip_path = dir.path().join("empty.zip");
    File::create(&zip_path).unwrap();
    let result = list_archive_entries(&zip_path, "", None);
    assert!(result.is_err());
}

#[test]
fn test_list_archive_nonexistent() {
    let path = Path::new("/nonexistent/file.zip");
    let result = list_archive_entries(path, "", None);
    assert!(result.is_err());
}

#[test]
fn test_list_archive_unsupported_format() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("file.unsupported");
    fs::write(&path, b"dummy").unwrap();
    let result = list_archive_entries(&path, "", None);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Unsupported format"));
}

#[test]
fn test_entries_to_load_contexts() {
    let entries = vec![
        ArchiveEntry {
            name: "file.txt".to_string(),
            is_dir: false,
            size: 123,
            mtime: 456,
            inner_path: "file.txt".to_string(),
            child_count: 0,
            is_encrypted: false,
        },
        ArchiveEntry {
            name: "sub".to_string(),
            is_dir: true,
            size: 0,
            mtime: 0,
            inner_path: "sub".to_string(),
            child_count: 0,
            is_encrypted: false,
        },
    ];
    let archive_path = Path::new("/home/user/archive.zip");
    let contexts = entries_to_load_contexts(&entries, archive_path, true);
    assert_eq!(contexts.len(), 2);
    let c1 = &contexts[0];
    assert_eq!(c1.display_name, "file.txt");
    assert!(!c1.is_dir);
    assert_eq!(c1.size, 123);
    assert_eq!(c1.mtime, 456);
    assert!(c1.expand_labels);
    assert_eq!(c1.target_path, build_archive_uri(archive_path, "file.txt"));
    let c2 = &contexts[1];
    assert!(c2.is_dir);
    assert_eq!(c2.target_path, build_archive_uri(archive_path, "sub"));
}

#[test]
fn test_extract_zip_entry() {
    let (_dir, zip_path) = create_test_zip();
    let tmp = extract_entry_to_tempfile(&zip_path, "file1.txt", None).unwrap();
    let content = fs::read_to_string(tmp.path()).unwrap();
    assert_eq!(content, "Hello");
    tmp.keep().ok();
}

#[test]
fn test_extract_zip_entry_in_subdir() {
    let (_dir, zip_path) = create_test_zip();
    let tmp = extract_entry_to_tempfile(&zip_path, "dir/file2.txt", None).unwrap();
    let content = fs::read_to_string(tmp.path()).unwrap();
    assert_eq!(content, "World");
    tmp.keep().ok();
}

#[test]
fn test_extract_tar_entry() {
    let (_dir, tar_path) = create_test_tar();
    let tmp = extract_entry_to_tempfile(&tar_path, "file1.txt", None).unwrap();
    let content = fs::read_to_string(tmp.path()).unwrap();
    assert_eq!(content, "Hello");
    tmp.keep().ok();
}

#[test]
fn test_extract_tar_gz_entry() {
    let (_dir, path) = create_compressed_tar("gz", |data| {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    });
    let tmp = extract_entry_to_tempfile(&path, "file1.txt", None).unwrap();
    let content = fs::read_to_string(tmp.path()).unwrap();
    assert_eq!(content, "Hello");
    tmp.keep().ok();
}

#[test]
fn test_extract_entry_not_found() {
    let (_dir, zip_path) = create_test_zip();
    let result = extract_entry_to_tempfile(&zip_path, "nonexistent.txt", None);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

#[test]
fn test_extract_from_unsupported_archive() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("file.unsupported");
    fs::write(&path, b"dummy").unwrap();
    let result = extract_entry_to_tempfile(&path, "file", None);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Unsupported format"));
}

#[test]
fn test_parse_7z_iso_list_output_parsing() {
    use std::collections::HashMap;

    let mock_7z_output = "\
7-Zip 23.01 (x64)
Listing archive: test.iso

----------
Path = docs/manual.pdf
Size = 204800
Attributes = _
Modified = 2026-05-10 12:00:00

Path = images
Size = 0
Attributes = D

        ";

    let mut seen: HashMap<String, flux::services::archive::ArchiveEntry> = HashMap::new();
    let mut cur_path = String::new();
    let mut cur_size: u64 = 0;
    let mut cur_is_dir = false;
    let mut in_block = false;

    for line in mock_7z_output.lines() {
        let line = line.trim();
        if line.starts_with("----------") {
            in_block = true;
            continue;
        }
        if !in_block {
            continue;
        }
        if line.is_empty() {
            if !cur_path.is_empty() {
                seen.insert(
                    cur_path.clone(),
                    flux::services::archive::ArchiveEntry {
                        name: cur_path.clone(),
                        is_dir: cur_is_dir,
                        size: cur_size,
                        mtime: 0,
                        inner_path: cur_path.clone(),
                        child_count: 0,
                        is_encrypted: false,
                    },
                );
            }
            cur_path.clear();
            cur_size = 0;
            cur_is_dir = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix("Path = ") {
            cur_path = rest.replace('\\', "/");
        } else if let Some(rest) = line.strip_prefix("Size = ") {
            cur_size = rest.trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("Attributes = ") {
            cur_is_dir = rest.trim_start().starts_with('D');
        }
    }

    assert_eq!(seen.len(), 2);
    assert_eq!(seen.get("docs/manual.pdf").unwrap().size, 204800);
    assert!(seen.get("images").unwrap().is_dir);
}
