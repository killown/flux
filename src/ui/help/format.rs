use crate::domain::model::Config;
use crate::i18n::tr;

/// Convert a config shortcut string (e.g. `<Primary>F7`) into a human-readable
/// keycap label (e.g. `Ctrl + F7`). Falls back to `default` when the config
/// value is absent or empty.
pub fn format_shortcut(shortcut: Option<String>, default: &str) -> String {
    let raw = shortcut.unwrap_or_else(|| default.to_string());
    if raw.trim().is_empty() {
        return default.to_string();
    }
    if raw.trim() == crate::ui::keymap::DISABLED {
        return tr("Disabled");
    }
    raw.split('|')
        .map(format_chord)
        .collect::<Vec<_>>()
        .join(" / ")
}

/// Formats a single accelerator chord (no `|` alternatives), e.g.
/// `<Primary><Shift>f` becomes `Ctrl + Shift + F`.
///
/// Modifier names are matched case-insensitively, unknown `<...>` prefixes are
/// left untouched.
fn format_chord(chord: &str) -> String {
    let mut rest = chord.trim();
    let mut parts: Vec<String> = Vec::new();

    while let Some(stripped) = rest.strip_prefix('<') {
        let Some(end) = stripped.find('>') else {
            break;
        };
        let label = match stripped[..end].to_ascii_lowercase().as_str() {
            "primary" | "control" | "ctrl" => "Ctrl",
            "alt" | "mod1" => "Alt",
            "shift" => "Shift",
            "super" | "mod4" => "Super",
            "meta" => "Meta",
            _ => break,
        };
        parts.push(label.to_string());
        rest = &stripped[end + 1..];
    }

    parts.push(format_key(rest));
    parts.join(" + ")
}

/// Converts a GDK key name into its keycap label (`Return` -> `Enter`, lone
/// letters are uppercased). Names without a special mapping pass through.
fn format_key(key: &str) -> String {
    match key {
        "Return" => "Enter".to_string(),
        "BackSpace" => "Backspace".to_string(),
        "slash" => "/".to_string(),
        "backslash" => "\\".to_string(),
        "space" => "Space".to_string(),
        _ if key.chars().count() == 1 => key.to_uppercase(),
        _ => key.to_string(),
    }
}

/// Convenience: format a shortcut field off a `Config`.
pub fn sc(
    config: &Config,
    field: impl Fn(&crate::domain::model::ShortcutsConfig) -> Option<String>,
    default: &str,
) -> String {
    format_shortcut(field(&config.shortcuts), default)
}

#[cfg(test)]
mod tests {
    use super::format_shortcut;

    #[test]
    fn renders_modifier_chords() {
        assert_eq!(
            format_shortcut(Some("<Primary><Shift>f".into()), ""),
            "Ctrl + Shift + F"
        );
        assert_eq!(
            format_shortcut(Some("<ctrl><alt>slash".into()), ""),
            "Ctrl + Alt + /"
        );
        assert_eq!(
            format_shortcut(Some("<Super>Return".into()), ""),
            "Super + Enter"
        );
    }

    #[test]
    fn renders_alternatives_and_defaults() {
        assert_eq!(
            format_shortcut(Some("<ctrl><shift>z|<ctrl>y".into()), ""),
            "Ctrl + Shift + Z / Ctrl + Y"
        );
        assert_eq!(format_shortcut(None, "Ctrl + S"), "Ctrl + S");
        assert_eq!(format_shortcut(Some("  ".into()), "F5"), "F5");
    }
}
