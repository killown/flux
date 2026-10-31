//! Hardcoded fallback shortcuts.
//!
//! These are used whenever `config.shortcuts.<field>` is `None` or empty.
//! `KeyMap::new` prefers whatever the user configured, and falls back here.

pub const HOME: &str = "<ctrl>Home";
pub const QUIT: &str = "<ctrl>q";
pub const OPEN: &str = "Return";
pub const DELETE: &str = "Delete";
pub const BACK: &str = "<alt>Left";
pub const FORWARD: &str = "<alt>Right";
pub const REFRESH: &str = "F5";
// Defined in `domain` because `utils::search::doc` must not import `ui`.
pub use crate::domain::model::constants::SEARCH_SHORTCUT as SEARCH;
pub const PROPERTIES: &str = "<ctrl>i";
pub const TOGGLE_HIDDEN: &str = "<ctrl>h";
pub const SETTINGS: &str = "F10";
pub const MENU_EDITOR: &str = "F9";
pub const ROOT: &str = "slash";
pub const CHANGE_ICON: &str = "F3";
pub const RESET_ICON: &str = "<ctrl>F3";
pub const TOGGLE_TERMINAL: &str = "F4";
pub const TOGGLE_HEADER: &str = "F6";
pub const TOGGLE_FOLDERS_FIRST: &str = "F3";
pub const NEW_TAB: &str = "<ctrl>t";
pub const CLOSE_TAB: &str = "<ctrl>w";
pub const NEXT_TAB: &str = "<ctrl>Tab";
pub const PREV_TAB: &str = "<ctrl><shift>Tab";
pub const TOGGLE_STATUSBAR: &str = "F7";
pub const TOGGLE_SIDEBAR: &str = "F8";
pub const TOGGLE_FILTER_BAR: &str = "<ctrl><shift>f";
pub const CYCLE_FILTER_MODE: &str = "<ctrl><shift>e";
pub const FILTER_BY_MIME: &str = "<ctrl>m";
pub const TOGGLE_WIDE_PREVIEW: &str = "F11";
pub const OPEN_DEBUG_WINDOW: &str = "<ctrl><shift>F7";
pub const UNDO: &str = "<ctrl>z";
pub const REDO: &str = "<ctrl><shift>z|<ctrl>y";
pub const TOGGLE_TAG_PANEL: &str = "<ctrl><shift>t";
pub const TOGGLE_ACTIONS_PANEL: &str = "<ctrl><shift>a";
pub const TOGGLE_CONNECT_PANEL: &str = "<ctrl><shift>l";
pub const TOGGLE_LOCATION_PANEL: &str = "<ctrl>l";
pub const TOGGLE_LIST_MODE: &str = "<ctrl><shift>comma|<ctrl>less";
pub const COPY: &str = "<ctrl>c";
pub const CUT: &str = "<ctrl>x";
pub const PASTE: &str = "<ctrl>v";
pub const CYCLE_SORT: &str = "<ctrl>s";
pub const TOGGLE_SORT_ORDER: &str = "<ctrl><shift>s";
pub const CYCLE_BY_LETTER_REANCHOR: &str = "<ctrl><alt>slash";
pub const CYCLE_BY_LETTER_CANCEL: &str = "<ctrl><alt>backslash";
pub const FOCUS_PANE_LEFT: &str = "<ctrl><alt>Left";
pub const FOCUS_PANE_RIGHT: &str = "<ctrl><alt>Right";
