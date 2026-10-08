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
pub const SEARCH: &str = "<ctrl>f";
pub const PROPERTIES: &str = "<ctrl>i";
pub const TOGGLE_HIDDEN: &str = "<ctrl>h";
pub const SETTINGS: &str = "F10";
pub const MENU_EDITOR: &str = "F9";
pub const ROOT: &str = "slash";
pub const CHANGE_ICON: &str = "F3";
pub const RESET_ICON: &str = "<ctrl>F3";
pub const TOGGLE_TERMINAL: &str = "F4";
pub const TOGGLE_HEADER: &str = "F6";
pub const TOGGLE_FOLDERS_FIRST: &str = "F7";
pub const NEW_TAB: &str = "<ctrl>t";
pub const CLOSE_TAB: &str = "<ctrl>w";
pub const NEXT_TAB: &str = "<ctrl>Tab";
pub const PREV_TAB: &str = "<ctrl><shift>Tab";
