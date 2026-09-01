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
        self.config
            .ui
            .file_icons
            .remove(&path.to_string_lossy().to_string());
        utils::save_config(&self.config);
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

    /// Displays a modal prompt to create a new sidebar section header.
    pub fn show_prompt_new_sidebar_section(&self, sender: &relm4::AsyncComponentSender<Self>) {
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let dialog = gtk::MessageDialog::new(
            parent.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Other,
            gtk::ButtonsType::None,
            crate::i18n::tr("New Section"),
        );
        dialog.set_secondary_text(Some(&crate::i18n::tr("Enter a title for the new section:")));

        dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
        let ok_btn = dialog.add_button(&crate::i18n::tr("Create"), gtk::ResponseType::Ok);
        ok_btn.style_context().add_class("suggested-action");
        dialog.set_default_response(gtk::ResponseType::Ok);

        let entry = gtk::Entry::builder()
            .placeholder_text(crate::i18n::tr("Section title"))
            .activates_default(true)
            .margin_top(8)
            .margin_bottom(4)
            .margin_start(16)
            .margin_end(16)
            .build();
        entry.connect_map(|e| {
            e.grab_focus();
        });
        dialog.content_area().append(&entry);
        dialog.present();

        let entry_clone = entry.clone();
        dialog.connect_response(move |dlg, resp| {
            if resp == gtk::ResponseType::Ok {
                let title = entry_clone.text().trim().to_string();
                if !title.is_empty() {
                    s.input(AppMsg::AddSidebarSection(title));
                }
            }
            dlg.close();
        });
    }

    /// Displays a modal prompt to rename an existing sidebar section header.
    ///
    /// Keyed by `old_name` since section labels have no path.
    pub fn show_prompt_sidebar_rename_section(
        &self,
        old_name: String,
        current_name: String,
        sender: &relm4::AsyncComponentSender<Self>,
    ) {
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let dialog = gtk::MessageDialog::new(
            parent.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Other,
            gtk::ButtonsType::None,
            "",
        );

        let title_markup = format!(
            "<span weight=\"bold\" size=\"medium\">{}</span>",
            crate::i18n::tr("Rename Section")
        );
        dialog.set_markup(&title_markup);
        dialog.set_secondary_text(Some(&crate::i18n::tr(
            "Enter a new title for this section:",
        )));

        let message_area = dialog.message_area();
        message_area.set_margin_top(12);
        message_area.set_margin_bottom(6);
        message_area.set_margin_start(12);
        message_area.set_margin_end(12);

        // Prevent GtkMessageDialog's primary title label from line-wrapping
        let mut child = message_area.first_child();
        while let Some(w) = child {
            if let Some(label) = w.downcast_ref::<gtk::Label>() {
                label.set_wrap(false);
                label.set_ellipsize(gtk::pango::EllipsizeMode::None);
                break;
            }
            child = w.next_sibling();
        }

        dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
        let ok_btn = dialog.add_button(&crate::i18n::tr("Rename"), gtk::ResponseType::Ok);
        ok_btn.style_context().add_class("suggested-action");
        dialog.set_default_response(gtk::ResponseType::Ok);

        let entry = gtk::Entry::builder()
            .text(&current_name)
            .activates_default(true)
            .margin_top(8)
            .margin_bottom(4)
            .margin_start(16)
            .margin_end(16)
            .build();
        entry.select_region(0, -1);
        entry.connect_map(|e| {
            e.grab_focus();
        });
        dialog.content_area().append(&entry);
        dialog.present();

        let entry_clone = entry.clone();
        dialog.connect_response(move |dlg, resp| {
            if resp == gtk::ResponseType::Ok {
                let new_name = entry_clone.text().trim().to_string();
                if !new_name.is_empty() && new_name != old_name {
                    s.input(AppMsg::RenameSidebarSection {
                        old_name: old_name.clone(),
                        new_name,
                    });
                }
            }
            dlg.close();
        });
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
