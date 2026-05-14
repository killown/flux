use crate::model::ShortcutsConfig;

/// Hardcoded fallback shortcuts.
pub mod constants {
    pub const QUIT: &str = "<ctrl>q";
    pub const OPEN: &str = "Return";
    pub const DELETE: &str = "Delete";
    pub const BACK: &str = "<alt>Left";
    pub const FORWARD: &str = "<alt>Right";
    pub const REFRESH: &str = "F5";
    pub const SEARCH: &str = "<ctrl>f";
    pub const PROPERTIES: &str = "<ctrl>i";
    pub const TOGGLE_HIDDEN: &str = "<ctrl>h";
    pub const SETTINGS: &str = "F10";
    pub const MENU_EDITOR: &str = "F9";
    pub const ROOT: &str = "slash";
    pub const CHANGE_ICON: &str = "F3";
    pub const RESET_ICON: &str = "<ctrl>F3";
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
    pub settings: gtk::ShortcutTrigger,
    pub menu_editor: gtk::ShortcutTrigger,
    pub root: gtk::ShortcutTrigger,
    pub change_icon: gtk::ShortcutTrigger,
    pub reset_icon: gtk::ShortcutTrigger,
}

impl KeyMap {
    pub fn new(config: &ShortcutsConfig) -> Self {
        //println!("[DEBUG] Shortcuts passed from TOML parser: {:#?}", config);

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
            settings: parse_trigger(&config.settings, constants::SETTINGS),
            menu_editor: parse_trigger(&config.menu_editor, constants::MENU_EDITOR),
            root: parse_trigger(&config.root, constants::ROOT),
            change_icon: parse_trigger(&config.change_icon, constants::CHANGE_ICON),
            reset_icon: parse_trigger(&config.reset_icon, constants::RESET_ICON),
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
            // Try the default, then a minimal safe fallback
            gtk::ShortcutTrigger::parse_string(default)
                .or_else(|| gtk::ShortcutTrigger::parse_string("Escape")) // Use a known-valid key
                .expect("Failed to parse fallback shortcut 'Escape'")
        }
    }
}
