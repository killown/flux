use std::path::{Path, PathBuf};

fn is_protected_target(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    if path_str.contains("://") {
        if let Some((_, after_scheme)) = path_str.split_once("://") {
            let inner_path = after_scheme
                .find('/')
                .map(|i| &after_scheme[i..])
                .unwrap_or("/");
            if inner_path.is_empty() || inner_path == "/" {
                return true;
            }
        }
        return false;
    }

    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    if resolved == Path::new("/") {
        return true;
    }

    if let Some(home) = dirs::home_dir() {
        if let Ok(canon_home) = home.canonicalize() {
            if resolved == canon_home {
                return true;
            }
        } else if resolved == home {
            return true;
        }
    }

    let protected_system_paths = [
        "/boot",
        "/dev",
        "/etc",
        "/lost+found",
        "/media",
        "/mnt",
        "/proc",
        "/root",
        "/run",
        "/run/media",
        "/sys",
        "/tmp",
        "/usr",
        "/var",
    ];

    for sys_path in protected_system_paths {
        if resolved == Path::new(sys_path) {
            return true;
        }
    }

    false
}

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

#[test]
fn test_protected_target_root() {
    assert!(is_protected_target(Path::new("/")));
    assert!(is_protected_target(Path::new("/./")));
}

#[test]
fn test_protected_target_home_directory() {
    if let Some(home) = dirs::home_dir() {
        assert!(is_protected_target(&home));
    }
}

#[test]
fn test_protected_target_system_directories() {
    let system_paths = [
        "/boot",
        "/dev",
        "/etc",
        "/media",
        "/mnt",
        "/proc",
        "/root",
        "/run",
        "/run/media",
        "/sys",
        "/tmp",
        "/usr",
        "/var",
    ];

    for sys_path in system_paths {
        assert!(
            is_protected_target(Path::new(sys_path)),
            "Path '{}' must be protected from deletion",
            sys_path
        );
    }
}

#[test]
fn test_protected_target_network_roots() {
    let network_roots = [
        "smb://server/",
        "smb://192.168.1.100",
        "sftp://user@example.com/",
        "sftp://example.com",
        "ftp://ftp.server.org/",
        "dav://webdav.server.com/",
    ];

    for root_uri in network_roots {
        assert!(
            is_protected_target(Path::new(root_uri)),
            "Network root URI '{}' must be protected from deletion",
            root_uri
        );
    }
}

#[test]
fn test_unprotected_regular_files_and_subdirectories() {
    let valid_deletable_paths = [
        "/tmp/flux_test_file.txt",
        "/tmp/sub_folder/nested_file.bin",
        "smb://server/share/subfolder/file.txt",
        "sftp://example.com/home/user/document.pdf",
    ];

    for target in valid_deletable_paths {
        assert!(
            !is_protected_target(Path::new(target)),
            "Target path '{}' should be allowed for deletion",
            target
        );
    }
}

#[test]
fn test_delete_selection_skips_protected_items() {
    let mock_selection = vec![
        PathBuf::from("/"),
        PathBuf::from("/etc"),
        PathBuf::from("/tmp/safe_to_delete.txt"),
    ];

    let mut filtered_for_deletion = Vec::new();
    for path in mock_selection {
        if is_protected_target(&path) {
            continue;
        }
        filtered_for_deletion.push(path);
    }

    assert_eq!(filtered_for_deletion.len(), 1);
    assert_eq!(
        filtered_for_deletion[0],
        PathBuf::from("/tmp/safe_to_delete.txt")
    );
}
