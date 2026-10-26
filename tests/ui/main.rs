//! Tests for `flux::ui` (widgets, dialogs, menus, panels).

mod undo_redo;

mod dialogs {
    mod conflict;
    mod file_properties;
    mod help_window;
    mod location;
    mod network;
    mod settings_window;
}

mod menus {
    mod context;
    mod editor;
    mod secondary;
}

mod panels {
    mod preview_theme;
}

// `app_view`, `inputs`, `ui_components`, `help_window`, `location`,
// `file_properties` re-implement the logic they assert on instead of calling
// into `flux`.
mod widgets {
    mod app_view;
    mod components;
    mod init_components;
    mod inputs;
    mod keymap;
    mod ui_components;
}
