use crate::model::{AppMsg, FluxApp, SortBy};
use crate::utils;
use relm4::prelude::*;

impl FluxApp {
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
        //WARNING: DO NOT REMOVE THE clear(). The folder cache stores pre-filtered items (dotfiles excluded at load time),
        // so toggling visibility without clearing it causes Ctrl+H to have no effect.
        self.folder_cache.clear();
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

    pub fn handle_set_scale_font_with_icons(
        &mut self,
        val: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config.ui.scale_font_with_icons = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_hidden_extensions(
        &mut self,
        exts: Vec<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config.ui.hidden_extensions = exts;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
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

    pub fn handle_set_show_empty_dir_emblem(&mut self, val: bool) {
        self.config.ui.show_empty_dir_emblem = val;
        utils::save_config(&self.config);
    }

    pub fn handle_toggle_current_folders_first(
        &mut self,
        sender: &relm4::AsyncComponentSender<Self>,
    ) {
        let key = self.current_path.to_string_lossy().to_string();
        let current_val = self
            .config
            .ui
            .current_folders_first
            .get(&key)
            .copied()
            .unwrap_or(self.config.ui.folders_first);

        let new_val = !current_val;
        self.config.ui.current_folders_first.insert(key, new_val);

        let sort_col = match self.sort_by {
            crate::model::SortBy::Name => "Name",
            crate::model::SortBy::Date => "Date",
            crate::model::SortBy::Size => "Size",
            crate::model::SortBy::Type => "Type",
        };

        let _ = self.state_db.save_view(
            &self.current_path,
            sort_col,
            !self.sort_ascending,
            self.current_icon_size as u32,
            new_val,
        );

        crate::utils::save_config(&self.config);
        self.folder_cache.remove(&self.current_path);
        self.load_path(self.current_path.clone(), sender);
    }

    pub fn handle_set_show_symlink_emblem(
        &mut self,
        val: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config.ui.show_symlink_emblem = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }
}
