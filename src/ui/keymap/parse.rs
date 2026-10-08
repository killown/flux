use super::constants;

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
                    // Cannot happen in practice - `Escape` always parses -
                    // but avoids a panic in the pathological case.
                    let _ = constants::QUIT;
                    gtk::ShortcutTrigger::parse_string("<ctrl>q")
                        .expect("even <ctrl>q failed to parse - GTK is broken")
                })
        }
    }
}
