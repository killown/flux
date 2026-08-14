use flux::model::MenuEntry;

fn split_mime_cmd(input: &str) -> Option<(String, String, Option<String>)> {
    let input = input.trim();
    let (mime, rest) = input.strip_prefix('"')?.split_once('"')?;
    let second = rest.trim().strip_prefix(',')?.trim();
    let (cmd, after_cmd) = second.strip_prefix('"')?.split_once('"')?;
    let toast = after_cmd
        .trim()
        .strip_prefix(',')
        .and_then(|s| s.trim().strip_prefix('"'))
        .and_then(|s| s.strip_suffix('"'))
        .map(|s| s.to_string());
    Some((mime.to_string(), cmd.to_string(), toast))
}

#[test]
fn test_split_mime_cmd_with_toast() {
    let input = r#""all", "builtin::copy", "Copied to clipboard""#;
    let (mime, cmd, toast) = split_mime_cmd(input).expect("must parse");

    assert_eq!(mime, "all");
    assert_eq!(cmd, "builtin::copy");
    assert_eq!(toast.as_deref(), Some("Copied to clipboard"));
}

#[test]
fn test_split_mime_cmd_without_toast() {
    let input = r#""image/*", "eog {path}""#;
    let (mime, cmd, toast) = split_mime_cmd(input).expect("must parse");

    assert_eq!(mime, "image/*");
    assert_eq!(cmd, "eog {path}");
    assert!(toast.is_none());
}

#[test]
fn test_menu_entry_config_line_serialization() {
    let entry = MenuEntry {
        label: "Copy Path".to_string(),
        submenu: Some("Tools".to_string()),
        mime_types: "all".to_string(),
        command: "builtin::copy_path".to_string(),
        toast: Some("Path Copied".to_string()),
        no_command_dialog: false,
    };

    let line = entry.to_config_line();

    assert!(line.contains("Tools > Copy Path"));
    assert!(line.contains("builtin::copy_path"));
    assert!(line.contains("Path Copied"));
}
