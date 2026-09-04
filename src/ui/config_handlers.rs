use crate::model::{AppMsg, FluxApp, SortBy};
use crate::utils;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn handle_set_max_content_search_results(
        &mut self,
        val: usize,
        _sender: &AsyncComponentSender<Self>,
    ) {
        self.config.ui.max_content_search_results = val;
        utils::save_config(&self.config);
    }

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

    // WARNING: lazy load mode will not work properly with custom icons
    pub fn handle_set_lazy_thumbnails(&mut self, val: bool) {
        self.config.ui.lazy_thumbnails = val;
        utils::save_config(&self.config);
    }

    pub fn handle_set_asc(&mut self, asc: bool, sender: &relm4::AsyncComponentSender<Self>) {
        self.sort_ascending = asc;
        let sort_col = match self.sort_by {
            SortBy::Name => "Name",
            SortBy::Date => "Date",
            SortBy::Size => "Size",
            SortBy::Type => "Type",
        };
        let _ = self.state_db.save_view(
            &self.current_path,
            sort_col,
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

    /// Persists and applies a new icon size for list mode view.
    pub fn handle_set_list_icon_size(&mut self, val: i32, sender: &AsyncComponentSender<Self>) {
        self.config.ui.list_icon_size = val;
        self.current_list_icon_size = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_sidebar_width(&mut self, val: i32) {
        self.config.ui.sidebar_width = val;
        utils::save_config(&self.config);
        if let Some(ref widget) = self.sidebar_widget {
            widget.set_width_request(val);
        }
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
        crate::utils::helpers::load_custom_css();
        self.terminal.apply_theme(&self.config.ui.terminal);
    }

    pub fn handle_set_default_sort(&mut self, sort: SortBy, sender: &AsyncComponentSender<Self>) {
        self.config.ui.default_sort = sort;
        self.sort_by = sort;
        utils::save_config(&self.config);
        let sort_col = match self.sort_by {
            SortBy::Name => "Name",
            SortBy::Date => "Date",
            SortBy::Size => "Size",
            SortBy::Type => "Type",
        };
        let _ = self.state_db.save_view(
            &self.current_path,
            sort_col,
            !self.sort_ascending,
            self.current_icon_size as u32,
            self.config.ui.folders_first,
        );
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_file_icon(
        &mut self,
        path: PathBuf,
        image_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let path_str = path.to_string_lossy().to_string();
        self.config
            .ui
            .file_icons
            .insert(path_str.clone(), image_path.to_string_lossy().to_string());
        utils::save_config(&self.config);

        if let Some(parent) = path.parent() {
            self.folder_cache.remove(parent);
        }
        self.folder_cache.remove(&path);

        // Live-update matching grid items instantly
        let new_icon = gtk::gio::Icon::for_string(&image_path.to_string_lossy())
            .unwrap_or_else(|_| utils::get_icon_for_path(&path, false));

        for i in 0..self.files.len() {
            if let Some(wrapper) = self.files.get(i) {
                let mut item = wrapper.borrow().clone();
                if item.path == path {
                    item.icon = new_icon.clone();
                    item.is_custom_icon = true;
                    *wrapper.borrow_mut() = item;
                }
            }
        }
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_reset_file_icon(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        let path_str = path.to_string_lossy().to_string();
        self.config.ui.file_icons.remove(&path_str);
        utils::save_config(&self.config);

        if let Some(parent) = path.parent() {
            self.folder_cache.remove(parent);
        }
        self.folder_cache.remove(&path);

        // Live-update matching grid items instantly back to default
        let default_icon = utils::get_icon_for_path(&path, false);
        for i in 0..self.files.len() {
            if let Some(wrapper) = self.files.get(i) {
                let mut item = wrapper.borrow().clone();
                if item.path == path {
                    item.icon = default_icon.clone();
                    item.is_custom_icon = false;
                    *wrapper.borrow_mut() = item;
                }
            }
        }
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

    pub fn handle_rename_sidebar_place(
        &mut self,
        path: PathBuf,
        new_name: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        let mut modified = false;

        for place in &mut self.config.sidebar {
            let expanded = utils::expand_path(&place.path);
            if expanded == path {
                place.name = new_name.clone();
                modified = true;
            }
        }

        let path_str = path.to_string_lossy().to_string();
        if let Some(device) = self.config.ui.device_renames.get_mut(&path_str) {
            device.name = new_name.clone();
            modified = true;
        } else {
            let path_trimmed = path_str.trim_end_matches('/').to_string();
            if let Some(device) = self.config.ui.device_renames.get_mut(&path_trimmed) {
                device.name = new_name.clone();
                modified = true;
            }
        }

        if modified {
            utils::save_config(&self.config);
            sender.input(AppMsg::RefreshSidebar);
        }
    }

    /// Removes a `kind = "label"` section header from config.sidebar by name and refreshes.
    pub fn handle_remove_sidebar_section(&mut self, name: String) {
        self.config
            .sidebar
            .retain(|e| !(e.kind.as_deref() == Some("label") && e.name == name));
        utils::save_config(&self.config);
        self.refresh_sidebar();
    }

    /// Appends a new `kind = "label"` section entry to the bottom of config.sidebar.
    pub fn handle_add_sidebar_section(&mut self, title: String) {
        let title = title.trim().to_string();
        if title.is_empty() {
            return;
        }
        self.config.sidebar.push(crate::model::CustomPlace {
            name: title,
            kind: Some("label".to_string()),
            icon: String::new(),
            path: String::new(),
        });
        utils::save_config(&self.config);
        self.refresh_sidebar();
    }

    pub fn handle_set_folder_icon(
        &mut self,
        path: PathBuf,
        icon_name: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        let path_str = path.to_string_lossy().to_string();
        if let Err(e) = self.state_db.set_folder_icon(&path_str, &icon_name) {
            eprintln!("[flux] Failed to save folder icon: {e}");
        }
        // Keep the in-memory cache in sync so the current session sees the change.
        self.config.ui.folder_icons.insert(path_str, icon_name);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_reset_folder_icon(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        let path_str = path.to_string_lossy().to_string();
        if let Err(e) = self.state_db.remove_folder_icon(&path_str) {
            eprintln!("[flux] Failed to remove folder icon: {e}");
        }
        self.config.ui.folder_icons.remove(&path_str);
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
