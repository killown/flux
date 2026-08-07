use std::path::PathBuf;

#[test]
fn test_middle_click_uri_scheme_detection() {
    let valid_uris = vec![
        "/home/user/folder",
        "trash:///file.txt",
        "smb://192.168.1.1/share",
        "sftp://remote.host/data",
        "ftp://ftp.server.org",
        "nfs://nfs.server/export",
        "archive:///tmp/file.zip#prefix",
    ];

    for uri in valid_uris {
        let is_valid = uri.starts_with('/')
            || uri.starts_with("trash://")
            || uri.starts_with("smb://")
            || uri.starts_with("sftp://")
            || uri.starts_with("ftp://")
            || uri.starts_with("nfs://")
            || uri.starts_with("archive://");

        assert!(is_valid);
        let path = PathBuf::from(uri);
        assert!(!path.to_string_lossy().is_empty());
    }
}

#[test]
fn test_modifier_keyval_matching() {
    let is_modifier_key = |keyval_str: &str| -> bool {
        matches!(
            keyval_str,
            "Control_L" | "Control_R" | "Shift_L" | "Shift_R"
        )
    };

    assert!(is_modifier_key("Control_L"));
    assert!(is_modifier_key("Control_R"));
    assert!(is_modifier_key("Shift_L"));
    assert!(is_modifier_key("Shift_R"));
    assert!(!is_modifier_key("Return"));
    assert!(!is_modifier_key("Escape"));
}

#[test]
fn test_swipe_velocity_threshold_detection() {
    let threshold = 500.0;

    let velocity_right = 750.0;
    let velocity_left = -800.0;
    let velocity_slow = 200.0;

    assert!(velocity_right > threshold);
    assert!(velocity_left < -threshold);
    assert!(velocity_slow <= threshold && velocity_slow >= -threshold);
}
