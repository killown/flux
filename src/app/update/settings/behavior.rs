use crate::model::{AppMsg, FluxApp};
use crate::utils;
use relm4::prelude::*;

impl FluxApp {
    pub fn handle_set_single_click(&mut self, val: bool) {
        self.config.ui.single_click = val;
        self.apply_single_click();
        utils::save_config(&self.config);
    }

    pub fn handle_set_auto_list_threshold(&mut self, val: usize) {
        self.config.ui.auto_list_threshold = val;
        utils::save_config(&self.config);
    }

    /// Toggles drawing browsable archives as folders in the file grid.
    ///
    /// Persists the setting, drops cached icons so the next paint re-resolves
    /// them, and refreshes the view. `AppMsg::Refresh` also clears the folder
    /// cache, so snapshots built with the old flag are not reused.
    pub fn handle_set_archives_as_folders(
        &mut self,
        val: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config.ui.archives_as_folders = val;
        utils::save_config(&self.config);
        crate::utils::icon::invalidate_themed_icon_cache();
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_toggle_single_click(&mut self) {
        self.config.ui.single_click = !self.config.ui.single_click;
        self.apply_single_click();
        crate::utils::save_config(&self.config);
    }

    /// Pushes `config.ui.single_click` to every tab's grid, so the setting
    /// applies to tabs that are already open and not only to new ones.
    fn apply_single_click(&self) {
        for tab in &self.tabs {
            tab.files
                .view
                .set_single_click_activate(self.config.ui.single_click);
        }
    }

    pub fn handle_set_shortcut(&mut self, key: String, val: Option<String>) {
        crate::hit!("handle_set_shortcut");
        match key.as_str() {
            "back" => self.config.shortcuts.back = val,
            "forward" => self.config.shortcuts.forward = val,
            "open" => self.config.shortcuts.open = val,
            "delete" => self.config.shortcuts.delete = val,
            "refresh" => self.config.shortcuts.refresh = val,
            "search" => self.config.shortcuts.search = val,
            "toggle_hidden" => self.config.shortcuts.toggle_hidden = val,
            "home" => self.config.shortcuts.home = val,
            "toggle_header" => self.config.shortcuts.toggle_header = val,
            "root" => self.config.shortcuts.root = val,
            "rename" => self.config.shortcuts.rename = val,
            "open_properties" => self.config.shortcuts.open_properties = val,
            "change_icon" => self.config.shortcuts.change_icon = val,
            "reset_icon" => self.config.shortcuts.reset_icon = val,
            "cycle_sort" => self.config.shortcuts.cycle_sort = val,
            "toggle_sort_order" => self.config.shortcuts.toggle_sort_order = val,
            "toggle_folders_first" => self.config.shortcuts.toggle_folders_first = val,
            "copy_path" => self.config.shortcuts.copy_path = val,
            "create_symlink" => self.config.shortcuts.create_symlink = val,
            "create_hardlink" => self.config.shortcuts.create_hardlink = val,
            "settings" => self.config.shortcuts.settings = val,
            "menu_editor" => self.config.shortcuts.menu_editor = val,
            "quit" => self.config.shortcuts.quit = val,
            "new_tab" => self.config.shortcuts.new_tab = val,
            "close_tab" => self.config.shortcuts.close_tab = val,
            "next_tab" => self.config.shortcuts.next_tab = val,
            "prev_tab" => self.config.shortcuts.prev_tab = val,
            "toggle_statusbar" => self.config.shortcuts.toggle_statusbar = val,
            "toggle_sidebar" => self.config.shortcuts.toggle_sidebar = val,
            "toggle_filter_bar" => self.config.shortcuts.toggle_filter_bar = val,
            "cycle_filter_mode" => self.config.shortcuts.cycle_filter_mode = val,
            "filter_by_mime" => self.config.shortcuts.filter_by_mime = val,
            "toggle_wide_preview" => self.config.shortcuts.toggle_wide_preview = val,
            "open_debug_window" => self.config.shortcuts.open_debug_window = val,
            "undo" => self.config.shortcuts.undo = val,
            "redo" => self.config.shortcuts.redo = val,
            "toggle_tag_panel" => self.config.shortcuts.toggle_tag_panel = val,
            "toggle_actions_panel" => self.config.shortcuts.toggle_actions_panel = val,
            "toggle_connect_panel" => self.config.shortcuts.toggle_connect_panel = val,
            "toggle_location_panel" => self.config.shortcuts.toggle_location_panel = val,
            "toggle_list_mode" => self.config.shortcuts.toggle_list_mode = val,
            "copy" => self.config.shortcuts.copy = val,
            "cut" => self.config.shortcuts.cut = val,
            "paste" => self.config.shortcuts.paste = val,
            "cycle_by_letter_reanchor" => self.config.shortcuts.cycle_by_letter_reanchor = val,
            "cycle_by_letter_cancel" => self.config.shortcuts.cycle_by_letter_cancel = val,
            "focus_pane_left" => self.config.shortcuts.focus_pane_left = val,
            "focus_pane_right" => self.config.shortcuts.focus_pane_right = val,
            "toggle_terminal" => self.config.shortcuts.toggle_terminal = val,
            _ => {}
        }
        utils::save_config(&self.config);
    }
}
