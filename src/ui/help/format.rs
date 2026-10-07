use crate::model::Config;

/// Convert a config shortcut string (e.g. `<Primary>F7`) into a human-readable
/// keycap label (e.g. `Ctrl + F7`). Falls back to `default` when the config
/// value is absent or empty.
pub fn format_shortcut(shortcut: Option<String>, default: &str) -> String {
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
}

/// Convenience: format a shortcut field off a `Config`.
pub fn sc(
    config: &Config,
    field: impl Fn(&crate::model::ShortcutsConfig) -> Option<String>,
    default: &str,
) -> String {
    format_shortcut(field(&config.shortcuts), default)
}
