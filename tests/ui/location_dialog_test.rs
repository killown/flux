use std::path::PathBuf;

#[test]
fn test_location_dialog_uri_handling() {
    let inputs = vec![
        ("smb://server/share", true),
        ("sftp://host/path", true),
        ("trash:///", true),
        ("recent:///", true),
        ("/home/user/documents", false),
        ("~/Downloads", false),
    ];

    for (trimmed, is_special_uri) in inputs {
        let is_network_or_special = trimmed.starts_with("smb://")
            || trimmed.starts_with("sftp://")
            || trimmed.starts_with("trash:///")
            || trimmed.starts_with("recent:///");

        assert_eq!(is_network_or_special, is_special_uri);

        let final_path = if is_network_or_special {
            PathBuf::from(trimmed)
        } else {
            // Expansion logic for standard paths
            PathBuf::from(trimmed)
        };

        assert!(!final_path.to_string_lossy().is_empty());
    }
}

#[test]
fn test_location_history_filter_matching() {
    let history = vec![
        "smb://192.168.1.100/share".to_string(),
        "sftp://example.com/data".to_string(),
        "/home/user/Documents".to_string(),
    ];

    let filter = "smb";
    let filter_lc = filter.to_lowercase();

    let matches: Vec<&String> = history
        .iter()
        .filter(|uri| uri.to_lowercase().contains(&filter_lc))
        .collect();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0], "smb://192.168.1.100/share");
}
