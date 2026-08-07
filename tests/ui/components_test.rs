use flux::model::PathSegment;
use flux::ui::SidebarPlace;
use std::path::PathBuf;

#[test]
fn test_path_segment_initialization() {
    let segment = PathSegment {
        name: "Projects".to_string(),
        path: PathBuf::from("/home/user/Projects"),
    };

    assert_eq!(segment.name, "Projects");
    assert_eq!(segment.path, PathBuf::from("/home/user/Projects"));
}

#[test]
fn test_sidebar_place_regular_item() {
    let place = SidebarPlace {
        name: "Documents".to_string(),
        icon: "folder-documents-symbolic".to_string(),
        path: PathBuf::from("/home/user/Documents"),
        is_mount: false,
        is_section_label: false,
    };

    assert_eq!(place.name, "Documents");
    assert_eq!(place.icon, "folder-documents-symbolic");
    assert!(!place.is_mount);
    assert!(!place.is_section_label);
}

#[test]
fn test_sidebar_place_mount_item() {
    let place = SidebarPlace {
        name: "External Drive".to_string(),
        icon: "drive-harddisk-symbolic".to_string(),
        path: PathBuf::from("/run/media/user/backup"),
        is_mount: true,
        is_section_label: false,
    };

    assert!(place.is_mount);
    assert!(!place.is_section_label);
    assert_eq!(place.path, PathBuf::from("/run/media/user/backup"));
}

#[test]
fn test_sidebar_place_section_label() {
    let place = SidebarPlace {
        name: "Bookmarks".to_string(),
        icon: String::new(),
        path: PathBuf::new(),
        is_mount: false,
        is_section_label: true,
    };

    assert!(place.is_section_label);
    assert!(!place.is_mount);
    assert_eq!(place.name, "Bookmarks");
}
