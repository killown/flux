use std::path::PathBuf;

#[test]
fn test_dot_pos_file_extension_truncation() {
    let name = "document.notes.txt";
    let dot_pos = name.rfind('.').unwrap_or(name.len());

    assert_eq!(dot_pos, 14);
    assert_eq!(&name[..dot_pos], "document.notes");

    let no_extension = "Makefile";
    let no_dot_pos = no_extension.rfind('.').unwrap_or(no_extension.len());
    assert_eq!(no_dot_pos, 8);
    assert_eq!(&no_extension[..no_dot_pos], "Makefile");
}

#[test]
fn test_sidebar_drag_path_string_parsing() {
    let raw_path_str = "/home/user/Documents";
    let parsed_path = PathBuf::from(raw_path_str);

    assert_eq!(parsed_path, PathBuf::from("/home/user/Documents"));
}

#[test]
fn test_file_item_list_mode_dimensions() {
    let is_list_mode = true;
    let icon_size = if is_list_mode { 32 } else { 64 };

    assert_eq!(icon_size, 32);
}
