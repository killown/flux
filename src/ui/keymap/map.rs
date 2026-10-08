use super::constants;
use super::parse::parse_trigger;
use crate::model::ShortcutsConfig;

/// A collection of resolved GTK `ShortcutTrigger`s.
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
    pub toggle_terminal: gtk::ShortcutTrigger,
    pub copy_path: gtk::ShortcutTrigger,
    pub create_symlink: gtk::ShortcutTrigger,
    pub create_hardlink: gtk::ShortcutTrigger,
    pub toggle_folders_first: gtk::ShortcutTrigger,
    pub toggle_header: gtk::ShortcutTrigger,
    pub home: gtk::ShortcutTrigger,
    pub new_tab: gtk::ShortcutTrigger,
    pub close_tab: gtk::ShortcutTrigger,
    pub next_tab: gtk::ShortcutTrigger,
    pub prev_tab: gtk::ShortcutTrigger,
}

impl KeyMap {
    pub fn new(config: &ShortcutsConfig) -> Self {
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
            toggle_terminal: parse_trigger(&None, constants::TOGGLE_TERMINAL),
            copy_path: parse_trigger(&config.copy_path, "<Primary><Shift>c"),
            create_symlink: parse_trigger(&config.create_symlink, "<Primary><Shift>v"),
            create_hardlink: parse_trigger(&config.create_hardlink, "<Primary><Alt><Shift>v"),
            toggle_folders_first: parse_trigger(
                &config.toggle_folders_first,
                constants::TOGGLE_FOLDERS_FIRST,
            ),
            toggle_header: parse_trigger(&config.toggle_header, constants::TOGGLE_HEADER),
            home: parse_trigger(&config.home, constants::HOME),
            new_tab: parse_trigger(&config.new_tab, constants::NEW_TAB),
            close_tab: parse_trigger(&config.close_tab, constants::CLOSE_TAB),
            next_tab: parse_trigger(&config.next_tab, constants::NEXT_TAB),
            prev_tab: parse_trigger(&config.prev_tab, constants::PREV_TAB),
        }
    }
}
