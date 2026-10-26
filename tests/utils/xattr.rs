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

#[test]
fn read_tags_strips_whitespace_in_entries() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("w.txt");
    File::create(&f).unwrap();
    let raw = b"  tag1  ,  tag2  ";
    if xattr::set(&f, XDG_TAGS_ATTR, raw).is_ok() {
        let parsed = read_tags(&f);
        assert_eq!(parsed, vec!["tag1", "tag2"]);
    }
}

#[test]
fn read_tags_ignores_consecutive_commas() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("comma.txt");
    File::create(&f).unwrap();
    let raw = b"tag1,,,tag2";
    if xattr::set(&f, XDG_TAGS_ATTR, raw).is_ok() {
        let parsed = read_tags(&f);
        assert_eq!(parsed, vec!["tag1", "tag2"]);
    }
}

#[test]
fn write_tags_unicode() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("u.txt");
    File::create(&f).unwrap();
    let tags = vec!["café".to_string(), "日本語".to_string()];
    if write_tags(&f, &tags).is_ok() {
        let parsed = read_tags(&f);
        assert_eq!(parsed, tags);
    }
}

#[test]
fn write_tags_preserves_order() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("order.txt");
    File::create(&f).unwrap();
    let tags = vec!["zeta".to_string(), "alpha".to_string(), "mu".to_string()];
    if write_tags(&f, &tags).is_ok() {
        let parsed = read_tags(&f);
        assert_eq!(parsed, tags);
    }
}

#[test]
fn read_tags_on_directory_returns_empty_or_some() {
    let dir = tempdir().unwrap();
    // Should not panic
    let _ = read_tags(dir.path());
}

#[test]
fn write_tags_then_clear_then_write() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("cycle.txt");
    File::create(&f).unwrap();
    if write_tags(&f, &["a".to_string()]).is_ok() {
        write_tags(&f, &[]).unwrap();
        write_tags(&f, &["b".to_string()]).unwrap();
        assert_eq!(read_tags(&f), vec!["b"]);
    }
}

#[test]
fn read_tags_strips_hash_from_entries() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("h.txt");
    File::create(&f).unwrap();
    let raw = b"#tag1,#tag2";
    if xattr::set(&f, XDG_TAGS_ATTR, raw).is_ok() {
        assert_eq!(read_tags(&f), vec!["tag1", "tag2"]);
    }
}

#[test]
fn read_tags_only_commas_returns_empty() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("only.txt");
    File::create(&f).unwrap();
    if xattr::set(&f, XDG_TAGS_ATTR, b",,,").is_ok() {
        assert!(read_tags(&f).is_empty());
    }
}

#[test]
fn read_tags_only_whitespace_returns_empty() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("ws.txt");
    File::create(&f).unwrap();
    if xattr::set(&f, XDG_TAGS_ATTR, b"   \n  ").is_ok() {
        assert!(read_tags(&f).is_empty());
    }
}

#[test]
fn write_tags_all_empty_removes_attribute() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("all_empty.txt");
    File::create(&f).unwrap();
    if write_tags(&f, &["a".to_string()]).is_ok() {
        write_tags(&f, &["".to_string(), "  ".to_string(), "#".to_string()]).unwrap();
        assert!(read_tags(&f).is_empty());
    }
}

#[test]
fn write_tags_removes_hash_prefix() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("clean.txt");
    File::create(&f).unwrap();
    if write_tags(&f, &["#x".to_string(), "#y".to_string()]).is_ok() {
        assert_eq!(read_tags(&f), vec!["x", "y"]);
    }
}

#[test]
fn write_tags_very_long_tag() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("long.txt");
    File::create(&f).unwrap();
    let long = "a".repeat(2000);
    if write_tags(&f, &[long.clone()]).is_ok() {
        assert_eq!(read_tags(&f), vec![long]);
    }
}
