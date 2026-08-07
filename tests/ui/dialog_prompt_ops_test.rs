use std::path::PathBuf;

#[test]
fn test_batch_creation_comma_parsing() {
    let input = " Folder A , Folder B,,Folder C ";
    let names: Vec<String> = input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    assert_eq!(names.len(), 3);
    assert_eq!(names[0], "Folder A");
    assert_eq!(names[1], "Folder B");
    assert_eq!(names[2], "Folder C");
}

#[test]
fn test_single_item_creation_path_resolution() {
    let current_path = PathBuf::from("/home/user/documents");
    let input = "New_File.txt";
    let names: Vec<String> = input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    assert_eq!(names.len(), 1);
    let target_path = current_path.join(&names[0]);
    assert_eq!(
        target_path,
        PathBuf::from("/home/user/documents/New_File.txt")
    );
}

#[test]
fn test_network_uri_formatting_for_new_entry() {
    let current_path = PathBuf::from("smb://192.168.1.100/share/");
    let name = "New_Folder";

    let uri = format!(
        "{}/{}",
        current_path.to_string_lossy().trim_end_matches('/'),
        name
    );

    assert_eq!(uri, "smb://192.168.1.100/share/New_Folder");
}

#[test]
fn test_icon_picker_filter_matching() {
    let icon_names = vec![
        "folder-documents",
        "folder-download",
        "user-home",
        "Folder-Pictures",
    ];

    let search_text = "folder";
    let matches: Vec<&&str> = icon_names
        .iter()
        .filter(|name| name.to_lowercase().contains(search_text))
        .collect();

    assert_eq!(matches.len(), 3);
    assert_eq!(*matches[0], "folder-documents");
    assert_eq!(*matches[1], "folder-download");
    assert_eq!(*matches[2], "Folder-Pictures");
}
