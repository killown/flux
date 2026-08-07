use crate::model::{AppMsg, FluxApp, SortBy};
use crate::utils;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

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

    pub fn handle_set_asc(&mut self, asc: bool, sender: &relm4::AsyncComponentSender<Self>) {
        self.sort_ascending = asc;
        let _ = self.state_db.save_view(
            &self.current_path,
            &format!("{:?}", self.sort_by),
            !self.sort_ascending,
            self.current_icon_size as u32,
            self.config.ui.folders_first,
        );
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_show_hidden(&mut self, val: bool, sender: &AsyncComponentSender<Self>) {
        self.show_hidden = val;
        self.config.ui.show_hidden_by_default = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_grid_spacing(&mut self, val: i32, sender: &AsyncComponentSender<Self>) {
        self.config.ui.grid_spacing = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_max_width_chars(&mut self, val: i32, sender: &AsyncComponentSender<Self>) {
        self.config.ui.max_width_chars = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_expand_labels(&mut self, val: bool, sender: &AsyncComponentSender<Self>) {
        self.config.ui.expand_labels = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_folders_first(&mut self, val: bool, sender: &AsyncComponentSender<Self>) {
        self.config.ui.folders_first = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_icon_size(&mut self, val: i32, sender: &AsyncComponentSender<Self>) {
        self.config.ui.default_icon_size = val;
        self.current_icon_size = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_sidebar_width(&mut self, val: i32) {
        self.config.ui.sidebar_width = val;
        utils::save_config(&self.config);
    }

    pub fn handle_set_show_csd(&mut self, val: bool) {
        self.config.ui.show_csd = val;
        utils::save_config(&self.config);
    }

    pub fn handle_set_show_xdg_dirs(&mut self, val: bool, sender: &AsyncComponentSender<Self>) {
        self.config.ui.show_xdg_dirs = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::RefreshSidebar);
    }

    pub fn handle_set_theme(&mut self, theme: Option<String>) {
        self.config.ui.theme = theme;
        utils::save_config(&self.config);
    }

    pub fn handle_set_default_sort(&mut self, sort: SortBy, sender: &AsyncComponentSender<Self>) {
        self.config.ui.default_sort = sort;
        self.sort_by = sort;
        utils::save_config(&self.config);
        let _ = self.state_db.save_view(
            &self.current_path,
            &format!("{:?}", self.sort_by),
            !self.sort_ascending,
            self.current_icon_size as u32,
            self.config.ui.folders_first,
        );
        sender.input(AppMsg::Refresh);
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
            _ => {}
        }
        utils::save_config(&self.config);
    }

    pub fn handle_set_maximized(&mut self, max: bool) {
        self.config.ui.start_maximized = max;
        utils::save_config(&self.config);

        let app = gtk::Application::default();
        if let Some(window) = app.active_window() {
            if max {
                window.maximize();
            } else {
                window.unmaximize();
            }
        }
    }

    pub fn handle_set_window_size(&mut self, width: Option<i32>, height: Option<i32>) {
        if let Some(w) = width {
            self.config.ui.startup_window_width = w;
        }
        if let Some(h) = height {
            self.config.ui.startup_window_height = h;
        }
        utils::save_config(&self.config);
    }

    pub fn handle_set_folder_icon(
        &mut self,
        path: PathBuf,
        icon_name: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config
            .ui
            .folder_icons
            .insert(path.to_string_lossy().to_string(), icon_name);
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_reset_folder_icon(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        self.config
            .ui
            .folder_icons
            .remove(&path.to_string_lossy().to_string());
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_show_thumbnails(&mut self, val: bool, sender: &AsyncComponentSender<Self>) {
        self.config.ui.show_thumbnails = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_thumbnail_type(
        &mut self,
        type_name: String,
        enabled: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        match type_name.as_str() {
            "images" => self.config.ui.thumbnail_types.images = enabled,
            "videos" => self.config.ui.thumbnail_types.videos = enabled,
            "fonts" => self.config.ui.thumbnail_types.fonts = enabled,
            "pdfs" => self.config.ui.thumbnail_types.pdfs = enabled,
            _ => {}
        }
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_show_recents(&mut self, val: bool, sender: &AsyncComponentSender<Self>) {
        self.config.ui.show_recents = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::RefreshSidebar);
    }

    pub fn handle_set_recents_row(&mut self, val: usize, sender: &AsyncComponentSender<Self>) {
        self.config.ui.recents_row = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::RefreshSidebar);
    }

    pub fn handle_set_terminal_config(
        &mut self,
        height: Option<i32>,
        font: Option<String>,
        fg: Option<String>,
        bg: Option<String>,
    ) {
        if let Some(h) = height {
            self.config.ui.terminal.height = h;
        }
        if let Some(f) = font {
            self.config.ui.terminal.font = f;
        }
        if let Some(c) = fg {
            self.config.ui.terminal.fg_color = c;
        }
        if let Some(c) = bg {
            self.config.ui.terminal.bg_color = c;
        }
        utils::save_config(&self.config);
    }
}

#[cfg(test)]
mod config_handlers_tests {
    use crate::model::{Config, UIConfig};
    use std::path::PathBuf;

    #[test]
    fn test_handle_set_shortcut_valid_and_invalid_keys() {
        let mut config = Config::default();

        // Simulate handle_set_shortcut matching logic
        let mut set_shortcut = |key: &str, val: Option<String>| match key {
            "back" => config.shortcuts.back = val,
            "forward" => config.shortcuts.forward = val,
            "open" => config.shortcuts.open = val,
            "delete" => config.shortcuts.delete = val,
            "refresh" => config.shortcuts.refresh = val,
            "search" => config.shortcuts.search = val,
            "toggle_hidden" => config.shortcuts.toggle_hidden = val,
            _ => {}
        };

        set_shortcut("back", Some("BackSpace".into()));
        set_shortcut("search", Some("<Control>f".into()));
        set_shortcut("non_existent_key", Some("Ctrl+X".into()));

        assert_eq!(config.shortcuts.back, Some("BackSpace".into()));
        assert_eq!(config.shortcuts.search, Some("<Control>f".into()));
    }

    #[test]
    fn test_handle_set_folder_icon_and_reset() {
        let mut ui_config = UIConfig::default();
        let folder_path = PathBuf::from("/home/user/Projects");
        let path_key = folder_path.to_string_lossy().to_string();

        // Simulate handle_set_folder_icon
        ui_config
            .folder_icons
            .insert(path_key.clone(), "folder-code".into());
        assert_eq!(
            ui_config.folder_icons.get(&path_key),
            Some(&"folder-code".to_string())
        );

        // Simulate handle_reset_folder_icon
        ui_config.folder_icons.remove(&path_key);
        assert!(!ui_config.folder_icons.contains_key(&path_key));
    }

    #[test]
    fn test_handle_set_thumbnail_type_matching() {
        let mut ui_config = UIConfig::default();

        let mut set_thumb_type = |type_name: &str, enabled: bool| match type_name {
            "images" => ui_config.thumbnail_types.images = enabled,
            "videos" => ui_config.thumbnail_types.videos = enabled,
            "fonts" => ui_config.thumbnail_types.fonts = enabled,
            "pdfs" => ui_config.thumbnail_types.pdfs = enabled,
            _ => {}
        };

        set_thumb_type("videos", false);
        set_thumb_type("pdfs", false);

        assert!(ui_config.thumbnail_types.images);
        assert!(!ui_config.thumbnail_types.videos);
        assert!(ui_config.thumbnail_types.fonts);
        assert!(!ui_config.thumbnail_types.pdfs);
    }

    #[test]
    fn test_handle_set_terminal_config_partial_updates() {
        let mut ui_config = UIConfig::default();

        let mut update_terminal =
            |h: Option<i32>, f: Option<String>, fg: Option<String>, bg: Option<String>| {
                if let Some(h_val) = h {
                    ui_config.terminal.height = h_val;
                }
                if let Some(f_val) = f {
                    ui_config.terminal.font = f_val;
                }
                if let Some(c_val) = fg {
                    ui_config.terminal.fg_color = c_val;
                }
                if let Some(c_val) = bg {
                    ui_config.terminal.bg_color = c_val;
                }
            };

        // Update only font and height
        update_terminal(Some(40), Some("Hack 12".into()), None, None);

        assert_eq!(ui_config.terminal.height, 40);
        assert_eq!(ui_config.terminal.font, "Hack 12");
        assert_eq!(ui_config.terminal.fg_color, "#E5E5E5"); // Remains default
    }
}
