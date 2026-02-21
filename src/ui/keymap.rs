use crate::model::ShortcutsConfig;

/// Hardcoded fallback shortcuts.
pub mod constants {
    pub const QUIT: &str = "<Control>q";
    pub const OPEN: &str = "Return";
    pub const DELETE: &str = "Delete";
    pub const BACK: &str = "<Alt>Left";
    pub const FORWARD: &str = "<Alt>Right";
    pub const REFRESH: &str = "F5";
    pub const SEARCH: &str = "<Control>f";
    pub const PROPERTIES: &str = "<Control>i";
    pub const TOGGLE_HIDDEN: &str = "<Control>h";
}

/// A collection of resolved GTK ShortcutTriggers.
#[derive(Debug)]
#[allow(dead_code)]
pub struct KeyMap {
    pub quit: gtk::ShortcutTrigger,
    pub open: gtk::ShortcutTrigger,
    pub delete: gtk::ShortcutTrigger,
    pub back: gtk::ShortcutTrigger,
    pub forward: gtk::ShortcutTrigger,
    pub refresh: gtk::ShortcutTrigger,
    pub search: gtk::ShortcutTrigger,
    pub properties: gtk::ShortcutTrigger,
    pub toggle_hidden: gtk::ShortcutTrigger,
}

impl KeyMap {
    pub fn new(config: &ShortcutsConfig) -> Self {
        println!("[DEBUG] Shortcuts passed from TOML parser: {:#?}", config);

        Self {
            quit: parse_trigger(&config.quit, constants::QUIT),
            open: parse_trigger(&config.open, constants::OPEN),
            delete: parse_trigger(&config.delete, constants::DELETE),
            back: parse_trigger(&config.back, constants::BACK),
            forward: parse_trigger(&config.forward, constants::FORWARD),
            refresh: parse_trigger(&config.refresh, constants::REFRESH),
            search: parse_trigger(&config.search, constants::SEARCH),
            properties: parse_trigger(&config.open_properties, constants::PROPERTIES),
            toggle_hidden: parse_trigger(&config.toggle_hidden, constants::TOGGLE_HIDDEN),
        }
    }
}

fn parse_trigger(user_val: &Option<String>, default: &str) -> gtk::ShortcutTrigger {
    let pattern = user_val.as_deref().unwrap_or(default);

    match gtk::ShortcutTrigger::parse_string(pattern) {
        Some(trigger) => trigger,
        None => {
            eprintln!(
                "[KEYMAP ERROR] GTK rejected shortcut '{}'. Falling back to '{}'.",
                pattern, default
            );
            gtk::ShortcutTrigger::parse_string(default)
                .expect("Static constant shortcut must be valid")
        }
    }
}
