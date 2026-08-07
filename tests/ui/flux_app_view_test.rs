use std::ffi::OsStr;
use std::path::PathBuf;

#[test]
fn test_window_title_formatting() {
    let get_title = |path: &PathBuf| -> String {
        let path_str = path.to_string_lossy();
        if path_str.starts_with("trash:///") {
            "Trash".to_string()
        } else {
            path.file_name()
                .unwrap_or_else(|| OsStr::new("/"))
                .to_string_lossy()
                .into_owned()
        }
    };

    assert_eq!(get_title(&PathBuf::from("trash:///")), "Trash");
    assert_eq!(
        get_title(&PathBuf::from("/home/user/Documents")),
        "Documents"
    );
    assert_eq!(get_title(&PathBuf::from("/")), "/");
}

#[test]
fn test_picked_path_extraction_logic() {
    let raw_widget_names = vec![
        "gtk-grid-view-child".to_string(),
        "/home/user/file.txt".to_string(),
        "gtk-grid-view".to_string(),
    ];

    let mut picked_path = None;
    for name in &raw_widget_names {
        if name.starts_with('/') || name.starts_with("trash://") {
            picked_path = Some(PathBuf::from(name));
            break;
        }
    }

    assert_eq!(picked_path, Some(PathBuf::from("/home/user/file.txt")));
}

#[test]
fn test_terminal_lines_height_calculation() {
    let paned_height = 800;
    let position = 560;
    let terminal_height = paned_height - position;

    let line_height_estimate = 24;
    let terminal_lines = terminal_height / line_height_estimate;

    assert_eq!(terminal_height, 240);
    assert_eq!(terminal_lines, 10);
}
