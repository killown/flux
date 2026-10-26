#[test]
fn test_format_shortcut_replacements() {
    let format_shortcut = |shortcut: Option<String>, default: &str| -> String {
        let raw = shortcut.unwrap_or_else(|| default.to_string());
        if raw.trim().is_empty() {
            return default.to_string();
        }
        raw.replace("<Primary>", "Ctrl + ")
            .replace("<Control>", "Ctrl + ")
            .replace("<control>", "Ctrl + ")
            .replace("<Alt>", "Alt + ")
            .replace("<Shift>", "Shift + ")
            .replace("Return", "Enter")
            .replace("BackSpace", "Backspace")
            .replace("slash", "/")
    };

    assert_eq!(
        format_shortcut(Some("<Primary><Shift>a".to_string()), "default"),
        "Ctrl + Shift + a"
    );
    assert_eq!(
        format_shortcut(Some("<Alt>Return".to_string()), "default"),
        "Alt + Enter"
    );
    assert_eq!(
        format_shortcut(Some("BackSpace".to_string()), "default"),
        "Backspace"
    );
    assert_eq!(format_shortcut(Some("slash".to_string()), "default"), "/");
    assert_eq!(
        format_shortcut(Some("   ".to_string()), "DefaultKey"),
        "DefaultKey"
    );
    assert_eq!(format_shortcut(None, "DefaultKey"), "DefaultKey");
}
