use flux::ui::keymap::constants;

#[test]
fn test_default_constants_validity() {
    assert_eq!(constants::QUIT, "<ctrl>q");
    assert_eq!(constants::OPEN, "Return");
    assert_eq!(constants::DELETE, "Delete");
    assert_eq!(constants::BACK, "<alt>Left");
    assert_eq!(constants::FORWARD, "<alt>Right");
    assert_eq!(constants::REFRESH, "F5");
    assert_eq!(constants::SEARCH, "<ctrl>f");
    assert_eq!(constants::PROPERTIES, "<ctrl>i");
    assert_eq!(constants::TOGGLE_HIDDEN, "<ctrl>h");
    assert_eq!(constants::SETTINGS, "F10");
    assert_eq!(constants::MENU_EDITOR, "F9");
    assert_eq!(constants::ROOT, "slash");
    assert_eq!(constants::CHANGE_ICON, "F3");
    assert_eq!(constants::RESET_ICON, "<ctrl>F3");
    assert_eq!(constants::TOGGLE_TERMINAL, "F4");
}

#[test]
fn test_shortcut_pattern_fallback_resolution() {
    let parse_pattern = |user_val: Option<&str>, default: &str| -> String {
        let pattern = user_val.unwrap_or(default);
        if pattern.is_empty() || pattern == "invalid_key_combo" {
            default.to_string()
        } else {
            pattern.to_string()
        }
    };

    assert_eq!(parse_pattern(Some("<ctrl>a"), constants::QUIT), "<ctrl>a");
    assert_eq!(parse_pattern(None, constants::QUIT), constants::QUIT);
    assert_eq!(
        parse_pattern(Some("invalid_key_combo"), constants::REFRESH),
        constants::REFRESH
    );
}
