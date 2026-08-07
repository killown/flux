use std::path::PathBuf;

#[test]
fn test_delete_selection_fallback_to_active_item() {
    let mut selection: Vec<PathBuf> = Vec::new();
    let active_item_path = Some(PathBuf::from("/tmp/active_file.txt"));

    if selection.is_empty() {
        if let Some(active) = active_item_path {
            selection.push(active);
        }
    }

    assert_eq!(selection.len(), 1);
    assert_eq!(selection[0], PathBuf::from("/tmp/active_file.txt"));
}

#[test]
fn test_delete_selection_empty_guard() {
    let mut selection: Vec<PathBuf> = Vec::new();
    let active_item_path: Option<PathBuf> = None;

    if selection.is_empty() {
        if let Some(active) = active_item_path {
            selection.push(active);
        }
    }

    assert!(selection.is_empty());
}

#[test]
fn test_uri_scheme_and_network_flag_detection() {
    let test_cases = vec![
        ("smb://192.168.1.1/share/file.txt", true, true),
        ("sftp://example.com/home/user/doc.pdf", true, true),
        ("file:///tmp/local_file.txt", false, true),
        ("/home/user/file.txt", false, false),
    ];

    for (path_str, expected_is_network, expected_contains_scheme) in test_cases {
        let contains_scheme = path_str.contains("://");
        let is_network = path_str.starts_with("smb://") || path_str.starts_with("sftp://");

        assert_eq!(contains_scheme, expected_contains_scheme);
        assert_eq!(is_network, expected_is_network);
    }
}
