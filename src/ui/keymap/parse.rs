use super::constants;

/// Sentinel value that unbinds a shortcut instead of rebinding it.
///
/// Used in `config.toml` as, for example, `delete = "disabled"`. It is checked
/// before `gtk::ShortcutTrigger::parse_string`, so it never reaches GTK and
/// never produces a `[KEYMAP ERROR]` fallback line.
///
/// Re-exported as `crate::ui::keymap::DISABLED`, `pub` is required for that
/// re-export to be legal, even though the only other reader is the settings
/// shortcut picker.
pub const DISABLED: &str = "disabled";

/// Parses a shortcut string, falling back to `default` (and finally to
/// `Escape`) if GTK rejects it.
pub(super) fn parse_trigger(user_val: &Option<String>, default: &str) -> gtk::ShortcutTrigger {
    let pattern = user_val.as_deref().unwrap_or(default);
    match gtk::ShortcutTrigger::parse_string(pattern) {
        Some(trigger) => trigger,
        None => {
            eprintln!(
                "[KEYMAP ERROR] GTK rejected shortcut '{}'. Falling back to '{}'.",
                pattern, default
            );
            gtk::ShortcutTrigger::parse_string(default)
                .or_else(|| gtk::ShortcutTrigger::parse_string("Escape"))
                .unwrap_or_else(|| {
                    let _ = constants::QUIT;
                    gtk::ShortcutTrigger::parse_string("<ctrl>q")
                        .expect("even <ctrl>q failed to parse - GTK is broken")
                })
        }
    }
}

/// Like [`parse_trigger_opt`], but returns the accelerator string that
/// `gtk::Application::set_accels_for_action` expects. Returns `None` when
/// the user set the shortcut to [`DISABLED`], meaning no accel is installed.
pub fn parse_accel_opt(user_val: &Option<String>, default: &str) -> Option<String> {
    if user_val.as_deref().map(str::trim) == Some(DISABLED) {
        return None;
    }
    Some(
        user_val
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(default)
            .to_string(),
    )
}

/// Like [`parse_trigger`], but returns `None` when the user set the shortcut
/// to [`DISABLED`], meaning the chord is unbound entirely.
///
/// Callers that register the trigger must skip it on `None` rather than
/// substitute a fallback, so that a disabled shortcut fires nothing at all.
pub(super) fn parse_trigger_opt(
    user_val: &Option<String>,
    default: &str,
) -> Option<gtk::ShortcutTrigger> {
    if user_val.as_deref().map(str::trim) == Some(DISABLED) {
        return None;
    }
    Some(parse_trigger(user_val, default))
}
