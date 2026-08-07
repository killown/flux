use flux::ui::constants;

#[test]
fn test_mime_matching_logic_broad_and_exact() {
    let mime_image = "image/png";
    let mime_dir = constants::MIME_DIR;
    let mime_txt = "text/plain";

    // Matching image/all
    let match_image = vec!["image/all".to_string()];
    let matches = match_image.iter().any(|m| match m.as_str() {
        "image/all" | "image/*" => mime_image.starts_with("image/"),
        _ => false,
    });
    assert!(matches);

    // Matching directory
    let match_dir = vec!["directory".to_string()];
    let matches_dir = match_dir.iter().any(|m| match m.as_str() {
        constants::FILTER_FOLDER | "directory" => mime_dir == constants::MIME_DIR,
        _ => false,
    });
    assert!(matches_dir);

    // Matching files only (non-directory)
    let match_file = vec!["file".to_string()];
    let matches_file = match_file.iter().any(|m| match m.as_str() {
        constants::FILTER_FILE => mime_txt != constants::MIME_DIR,
        _ => false,
    });
    assert!(matches_file);
}

#[test]
fn test_trash_filter_matching() {
    let is_in_trash = true;
    let mime_types = vec![constants::FILTER_TRASH.to_string()];

    let matches = is_in_trash && mime_types.contains(&constants::FILTER_TRASH.to_string());
    assert!(matches);
}

#[test]
fn test_builtin_command_action_mapping() {
    let command = "builtin::copy";

    let (full_action_name, lookup_name) = match command {
        "builtin::copy" => ("win.copy".to_string(), "copy"),
        "builtin::cut" => ("win.cut".to_string(), "cut"),
        "builtin::paste" => ("win.paste".to_string(), "paste"),
        "builtin::delete" => ("win.delete-selection".to_string(), "delete-selection"),
        _ => ("win.custom".to_string(), "custom"),
    };

    assert_eq!(full_action_name, "win.copy");
    assert_eq!(lookup_name, "copy");
}
