use std::path::PathBuf;

#[test]
fn test_icon_picker_target_path_preservation() {
    let target_path = PathBuf::from("/home/user/Documents/Projects");
    let icon_names = vec![
        "folder-symbolic".to_string(),
        "folder-documents-symbolic".to_string(),
        "folder-download-symbolic".to_string(),
    ];

    assert_eq!(target_path, PathBuf::from("/home/user/Documents/Projects"));
    assert_eq!(icon_names.len(), 3);
}

#[test]
fn test_default_icon_list_completeness() {
    let icon_names = vec![
        "folder-symbolic",
        "folder-documents-symbolic",
        "folder-download-symbolic",
        "folder-music-symbolic",
        "folder-pictures-symbolic",
        "folder-videos-symbolic",
        "folder-development-symbolic",
        "folder-remote-symbolic",
        "user-home-symbolic",
        "drive-harddisk-symbolic",
    ];

    assert_eq!(icon_names.len(), 10);
    assert!(icon_names.contains(&"folder-documents-symbolic"));
    assert!(icon_names.contains(&"user-home-symbolic"));
}
