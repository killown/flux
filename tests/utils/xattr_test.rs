use flux::utils::xattr::{read_tags, write_tags, XDG_TAGS_ATTR};
use std::fs::File;
use tempfile::tempdir;

#[test]
fn test_write_and_read_tags_roundtrip() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("tagged_file.txt");
    File::create(&file_path).unwrap();

    let tags = vec![
        "work".to_string(),
        "finance".to_string(),
        "2026".to_string(),
    ];
    let res = write_tags(&file_path, &tags);

    // Skip assertion if the backing filesystem doesn't support user xattrs (e.g. tmpfs without user_xattr)
    if res.is_ok() {
        let read_back = read_tags(&file_path);
        assert_eq!(read_back, tags);
    }
}

#[test]
fn test_write_tags_cleans_leading_hash_and_whitespace() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("clean_tags.txt");
    File::create(&file_path).unwrap();

    let tags = vec![
        "  #urgent  ".to_string(),
        "#project_alpha".to_string(),
        "".to_string(),
        "   ".to_string(),
    ];
    if write_tags(&file_path, &tags).is_ok() {
        let read_back = read_tags(&file_path);
        assert_eq!(
            read_back,
            vec!["urgent".to_string(), "project_alpha".to_string()]
        );
    }
}

#[test]
fn test_write_empty_tags_removes_xattr() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("clear_tags.txt");
    File::create(&file_path).unwrap();

    if write_tags(&file_path, &["initial".to_string()]).is_ok() {
        assert!(!read_tags(&file_path).is_empty());

        let res = write_tags(&file_path, &[]);
        assert!(res.is_ok());
        assert!(read_tags(&file_path).is_empty());
    }
}

#[test]
fn test_read_tags_from_non_existent_file() {
    let non_existent = std::path::PathBuf::from("/nonexistent/file_for_tags.txt");
    let tags = read_tags(non_existent);
    assert!(tags.is_empty());
}

#[test]
fn test_read_tags_parses_comma_and_newline_separators() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("raw_xattr.txt");
    File::create(&file_path).unwrap();

    let raw_data = b"tag1, #tag2\ntag3\n, #tag4";
    if xattr::set(&file_path, XDG_TAGS_ATTR, raw_data).is_ok() {
        let parsed = read_tags(&file_path);
        assert_eq!(parsed, vec!["tag1", "tag2", "tag3", "tag4"]);
    }
}
