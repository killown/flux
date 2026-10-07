use crate::model::FluxApp;
use crate::utils;

impl FluxApp {
    pub fn handle_set_single_click(&mut self, val: bool) {
        self.config.ui.single_click = val;
        self.files.view.set_single_click_activate(val);
        utils::save_config(&self.config);
    }

    pub fn handle_toggle_single_click(&mut self) {
        self.config.ui.single_click = !self.config.ui.single_click;
        self.files
            .view
            .set_single_click_activate(self.config.ui.single_click);
        crate::utils::save_config(&self.config);
    }

    pub fn handle_set_shortcut(&mut self, key: String, val: Option<String>) {
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
            _ => {}
        }
        utils::save_config(&self.config);
    }
}
