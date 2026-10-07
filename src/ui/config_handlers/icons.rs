use crate::model::{AppMsg, FluxApp};
use crate::utils;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
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
            .unwrap_or_else(|_| utils::icon::get_icon_for_path(&path, false));

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
        let default_icon = utils::icon::get_icon_for_path(&path, false);
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

    pub fn handle_set_auto_mime_body_color(
        &mut self,
        color: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config.ui.auto_mime_body_color = color;
        utils::save_config(&self.config);
        crate::services::loader::invalidate_extension_icon_cache();
        crate::utils::icon::invalidate_themed_icon_cache();
        self.load_path(self.current_path.clone(), sender);
    }

    pub fn handle_set_auto_mime_font_color(
        &mut self,
        color: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config.ui.auto_mime_font_color = color;
        utils::save_config(&self.config);
        crate::services::loader::invalidate_extension_icon_cache();
        crate::utils::icon::invalidate_themed_icon_cache();
        self.load_path(self.current_path.clone(), sender);
    }

    pub fn handle_set_auto_generate_mime_icons(
        &mut self,
        enabled: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config.ui.auto_generate_mime_icons = enabled;
        utils::save_config(&self.config);
        crate::services::loader::invalidate_extension_icon_cache();
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_auto_mime_accent_color(
        &mut self,
        color: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config.ui.auto_mime_accent_color = color;
        utils::save_config(&self.config);
        crate::services::loader::invalidate_extension_icon_cache();
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_auto_mime_font_size(
        &mut self,
        size: f64,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config.ui.auto_mime_font_size = size;
        utils::save_config(&self.config);
        crate::services::loader::invalidate_extension_icon_cache();
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_reset_extension_icon(
        &mut self,
        ext: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        let clean_ext = ext.trim_start_matches('.').to_ascii_lowercase();
        if let Some(custom_dir) =
            dirs::data_local_dir().map(|d| d.join("flux/icons/extensions/custom"))
        {
            for fmt in &["png", "svg", "webp", "jpg", "jpeg"] {
                let p = custom_dir.join(format!("{}.{}", clean_ext, fmt));
                let _ = std::fs::remove_file(p);
            }
        }

        crate::services::loader::invalidate_extension_icon_cache();
        crate::utils::icon::invalidate_themed_icon_cache();
        self.tabs[self.active_tab_index].files.clear();
        self.load_path(self.current_path.clone(), sender);
    }

    pub fn handle_trigger_reset_icon(&self, sender: &AsyncComponentSender<Self>) {
        let target = self
            .get_selected_path()
            .unwrap_or_else(|| self.current_path.clone());
        if target.is_dir() {
            sender.input(AppMsg::ResetFolderIcon(target.clone()));
            sender.input(AppMsg::ResetFileIcon(target));
        } else {
            sender.input(AppMsg::ResetFileIcon(target));
        }
    }

    pub fn handle_trigger_icon_picker(&self, sender: &AsyncComponentSender<Self>) {
        let target = self
            .get_selected_path()
            .unwrap_or_else(|| self.current_path.clone());
        if target.is_dir() {
            self.show_icon_picker(target, sender);
        }
    }

    pub fn handle_folder_icons_ready(
        &mut self,
        icons: std::collections::HashMap<String, String>,
        session: u64,
    ) {
        if session != self.load_id.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        for i in 0..self.tabs[self.active_tab_index].files.len() {
            let path_key_opt = self.tabs[self.active_tab_index]
                .files
                .get(i)
                .map(|w| w.borrow().path.to_string_lossy().to_string());
            if let Some(path_key) = path_key_opt {
                if let Some(icon_name) = icons.get(&path_key) {
                    if let Ok(icon) = gtk::gio::Icon::for_string(icon_name) {
                        let item_opt = self.tabs[self.active_tab_index].files.get(i).map(|w| {
                            let mut item = w.borrow().clone();
                            item.icon = icon.clone();
                            item.is_custom_icon = true;
                            item
                        });
                        if let Some(item) = item_opt {
                            self.tabs[self.active_tab_index].files.remove(i);
                            self.tabs[self.active_tab_index].files.insert(i, item);
                        }
                    }
                }
            }
        }
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
}
