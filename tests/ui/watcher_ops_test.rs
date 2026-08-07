use std::path::PathBuf;

#[test]
fn test_handle_file_deleted_index_match() {
    let mut files = vec!["documents", "image.png", "notes.txt"];
    let deleted_path = PathBuf::from("/home/user/image.png");

    if let Some(name) = deleted_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
    {
        if let Some(idx) = files.iter().position(|&item| item == name) {
            files.remove(idx);
        }
    }

    assert_eq!(files.len(), 2);
    assert_eq!(files, vec!["documents", "notes.txt"]);
}

#[test]
fn test_handle_file_deleted_non_existent() {
    let mut files = vec!["documents", "notes.txt"];
    let deleted_path = PathBuf::from("/home/user/missing_file.txt");

    if let Some(name) = deleted_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
    {
        if let Some(idx) = files.iter().position(|&item| item == name) {
            files.remove(idx);
        }
    }

    assert_eq!(files.len(), 2);
    assert_eq!(files, vec!["documents", "notes.txt"]);
}

#[test]
fn test_start_rename_toggle_editing_flag() {
    #[derive(Clone, Debug, PartialEq)]
    struct MockFileItem {
        path: PathBuf,
        is_editing: bool,
    }

    let mut files = vec![
        MockFileItem {
            path: PathBuf::from("/tmp/a.txt"),
            is_editing: false,
        },
        MockFileItem {
            path: PathBuf::from("/tmp/b.txt"),
            is_editing: false,
        },
    ];

    let target_path = PathBuf::from("/tmp/b.txt");

    if let Some(idx) = files.iter().position(|item| item.path == target_path) {
        let mut item = files[idx].clone();
        item.is_editing = true;
        files.remove(idx);
        files.insert(idx, item);
    }

    assert!(!files[0].is_editing);
    assert!(files[1].is_editing);
}
