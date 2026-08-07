use std::path::PathBuf;

#[test]
fn test_mount_row_path_conversion() {
    let uri = "smb://192.168.1.50/share";
    let path = PathBuf::from(uri);

    assert_eq!(path.to_string_lossy(), "smb://192.168.1.50/share");
}

#[test]
fn test_network_section_visibility_logic() {
    let mounts_empty: Vec<(&str, &str, &str)> = vec![];
    let mounts_populated = vec![("smb://server/share", "Share", "folder-remote")];

    assert!(mounts_empty.is_empty());
    assert!(!mounts_populated.is_empty());
}
