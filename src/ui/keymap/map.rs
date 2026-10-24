use super::constants;
use super::parse::parse_trigger_opt;
use crate::model::ShortcutsConfig;

/// A collection of resolved GTK `ShortcutTrigger`s.
///
/// Every field is an `Option`: `None` means the shortcut is disabled
/// (`config.toml` set it to `"disabled"`) and callers must skip it entirely
/// rather than substituting a fallback. See `parse_trigger_opt`.
#[derive(Debug)]
#[allow(dead_code)]
pub struct KeyMap {
    pub quit: Option<gtk::ShortcutTrigger>,
    pub open: Option<gtk::ShortcutTrigger>,
    pub delete: Option<gtk::ShortcutTrigger>,
    pub back: Option<gtk::ShortcutTrigger>,
    pub forward: Option<gtk::ShortcutTrigger>,
    pub refresh: Option<gtk::ShortcutTrigger>,
    pub search: Option<gtk::ShortcutTrigger>,
    pub properties: Option<gtk::ShortcutTrigger>,
    pub toggle_hidden: Option<gtk::ShortcutTrigger>,
    pub settings: Option<gtk::ShortcutTrigger>,
    pub menu_editor: Option<gtk::ShortcutTrigger>,
    pub root: Option<gtk::ShortcutTrigger>,
    pub change_icon: Option<gtk::ShortcutTrigger>,
    pub reset_icon: Option<gtk::ShortcutTrigger>,
    pub toggle_terminal: Option<gtk::ShortcutTrigger>,
    pub copy_path: Option<gtk::ShortcutTrigger>,
    pub create_symlink: Option<gtk::ShortcutTrigger>,
    pub create_hardlink: Option<gtk::ShortcutTrigger>,
    pub toggle_folders_first: Option<gtk::ShortcutTrigger>,
    pub toggle_header: Option<gtk::ShortcutTrigger>,
    pub home: Option<gtk::ShortcutTrigger>,
    pub new_tab: Option<gtk::ShortcutTrigger>,
    pub close_tab: Option<gtk::ShortcutTrigger>,
    pub next_tab: Option<gtk::ShortcutTrigger>,
    pub prev_tab: Option<gtk::ShortcutTrigger>,
    pub toggle_statusbar: Option<gtk::ShortcutTrigger>,
    pub toggle_sidebar: Option<gtk::ShortcutTrigger>,
    pub toggle_filter_bar: Option<gtk::ShortcutTrigger>,
    pub cycle_filter_mode: Option<gtk::ShortcutTrigger>,
    pub filter_by_mime: Option<gtk::ShortcutTrigger>,
    pub toggle_wide_preview: Option<gtk::ShortcutTrigger>,
    pub open_debug_window: Option<gtk::ShortcutTrigger>,
    pub undo: Option<gtk::ShortcutTrigger>,
    pub redo: Option<gtk::ShortcutTrigger>,
    pub toggle_tag_panel: Option<gtk::ShortcutTrigger>,
    pub toggle_actions_panel: Option<gtk::ShortcutTrigger>,
    pub toggle_connect_panel: Option<gtk::ShortcutTrigger>,
    pub toggle_location_panel: Option<gtk::ShortcutTrigger>,
    pub toggle_list_mode: Option<gtk::ShortcutTrigger>,
    pub copy: Option<gtk::ShortcutTrigger>,
    pub cut: Option<gtk::ShortcutTrigger>,
    pub paste: Option<gtk::ShortcutTrigger>,
    pub cycle_sort: Option<gtk::ShortcutTrigger>,
    pub toggle_sort_order: Option<gtk::ShortcutTrigger>,
    pub cycle_by_letter_reanchor: Option<gtk::ShortcutTrigger>,
    pub cycle_by_letter_cancel: Option<gtk::ShortcutTrigger>,
    pub focus_pane_left: Option<gtk::ShortcutTrigger>,
    pub focus_pane_right: Option<gtk::ShortcutTrigger>,
}

impl KeyMap {
    pub fn new(config: &ShortcutsConfig) -> Self {
        Self {
            quit: parse_trigger_opt(&config.quit, constants::QUIT),
            open: parse_trigger_opt(&config.open, constants::OPEN),
            delete: parse_trigger_opt(&config.delete, constants::DELETE),
            back: parse_trigger_opt(&config.back, constants::BACK),
            forward: parse_trigger_opt(&config.forward, constants::FORWARD),
            refresh: parse_trigger_opt(&config.refresh, constants::REFRESH),
            search: parse_trigger_opt(&config.search, constants::SEARCH),
            properties: parse_trigger_opt(&config.open_properties, constants::PROPERTIES),
            toggle_hidden: parse_trigger_opt(&config.toggle_hidden, constants::TOGGLE_HIDDEN),
            settings: parse_trigger_opt(&config.settings, constants::SETTINGS),
            menu_editor: parse_trigger_opt(&config.menu_editor, constants::MENU_EDITOR),
            root: parse_trigger_opt(&config.root, constants::ROOT),
            change_icon: parse_trigger_opt(&config.change_icon, constants::CHANGE_ICON),
            reset_icon: parse_trigger_opt(&config.reset_icon, constants::RESET_ICON),
            toggle_terminal: parse_trigger_opt(&config.toggle_terminal, constants::TOGGLE_TERMINAL),
            copy_path: parse_trigger_opt(&config.copy_path, "<Primary><Shift>c"),
            create_symlink: parse_trigger_opt(&config.create_symlink, "<Primary><Shift>v"),
            create_hardlink: parse_trigger_opt(&config.create_hardlink, "<Primary><Alt><Shift>v"),
            toggle_folders_first: parse_trigger_opt(
                &config.toggle_folders_first,
                constants::TOGGLE_FOLDERS_FIRST,
            ),
            toggle_header: parse_trigger_opt(&config.toggle_header, constants::TOGGLE_HEADER),
            home: parse_trigger_opt(&config.home, constants::HOME),
            new_tab: parse_trigger_opt(&config.new_tab, constants::NEW_TAB),
            close_tab: parse_trigger_opt(&config.close_tab, constants::CLOSE_TAB),
            next_tab: parse_trigger_opt(&config.next_tab, constants::NEXT_TAB),
            prev_tab: parse_trigger_opt(&config.prev_tab, constants::PREV_TAB),
            toggle_statusbar: parse_trigger_opt(
                &config.toggle_statusbar,
                constants::TOGGLE_STATUSBAR,
            ),
            toggle_sidebar: parse_trigger_opt(&config.toggle_sidebar, constants::TOGGLE_SIDEBAR),
            toggle_filter_bar: parse_trigger_opt(
                &config.toggle_filter_bar,
                constants::TOGGLE_FILTER_BAR,
            ),
            cycle_filter_mode: parse_trigger_opt(
                &config.cycle_filter_mode,
                constants::CYCLE_FILTER_MODE,
            ),
            filter_by_mime: parse_trigger_opt(&config.filter_by_mime, constants::FILTER_BY_MIME),
            toggle_wide_preview: parse_trigger_opt(
                &config.toggle_wide_preview,
                constants::TOGGLE_WIDE_PREVIEW,
            ),
            open_debug_window: parse_trigger_opt(
                &config.open_debug_window,
                constants::OPEN_DEBUG_WINDOW,
            ),
            undo: parse_trigger_opt(&config.undo, constants::UNDO),
            redo: parse_trigger_opt(&config.redo, constants::REDO),
            toggle_tag_panel: parse_trigger_opt(
                &config.toggle_tag_panel,
                constants::TOGGLE_TAG_PANEL,
            ),
            toggle_actions_panel: parse_trigger_opt(
                &config.toggle_actions_panel,
                constants::TOGGLE_ACTIONS_PANEL,
            ),
            toggle_connect_panel: parse_trigger_opt(
                &config.toggle_connect_panel,
                constants::TOGGLE_CONNECT_PANEL,
            ),
            toggle_location_panel: parse_trigger_opt(
                &config.toggle_location_panel,
                constants::TOGGLE_LOCATION_PANEL,
            ),
            toggle_list_mode: parse_trigger_opt(
                &config.toggle_list_mode,
                constants::TOGGLE_LIST_MODE,
            ),
            copy: parse_trigger_opt(&config.copy, constants::COPY),
            cut: parse_trigger_opt(&config.cut, constants::CUT),
            paste: parse_trigger_opt(&config.paste, constants::PASTE),
            cycle_sort: parse_trigger_opt(&config.cycle_sort, constants::CYCLE_SORT),
            toggle_sort_order: parse_trigger_opt(
                &config.toggle_sort_order,
                constants::TOGGLE_SORT_ORDER,
            ),
            cycle_by_letter_reanchor: parse_trigger_opt(
                &config.cycle_by_letter_reanchor,
                constants::CYCLE_BY_LETTER_REANCHOR,
            ),
            cycle_by_letter_cancel: parse_trigger_opt(
                &config.cycle_by_letter_cancel,
                constants::CYCLE_BY_LETTER_CANCEL,
            ),
            focus_pane_left: parse_trigger_opt(&config.focus_pane_left, constants::FOCUS_PANE_LEFT),
            focus_pane_right: parse_trigger_opt(
                &config.focus_pane_right,
                constants::FOCUS_PANE_RIGHT,
            ),
        }
    }
}
