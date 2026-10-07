use crate::model::{AppMsg, FluxApp};
use crate::utils;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn handle_set_sidebar_width(&mut self, val: i32) {
        let clamped = val.clamp(160, 500);
        self.config.ui.sidebar_width = clamped;
        utils::save_config(&self.config);
        if let Some(ref widget) = self.sidebar_widget {
            if let Some(box_container) = widget.downcast_ref::<gtk::Box>() {
                if let Some(first_child) = box_container.first_child() {
                    first_child.set_width_request(clamped);
                }
            } else {
                widget.set_width_request(clamped);
            }
        }
    }

    pub fn handle_set_show_xdg_dirs(&mut self, val: bool, sender: &AsyncComponentSender<Self>) {
        self.config.ui.show_xdg_dirs = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::RefreshSidebar);
    }

    pub fn handle_rename_sidebar_place(
        &mut self,
        path: PathBuf,
        new_name: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        let mut modified = false;

        for place in &mut self.config.sidebar {
            let expanded = Self::expand_path(&place.path);
            if expanded == path {
                place.name = new_name.clone();
                modified = true;
            }
        }

        let path_str = path.to_string_lossy().to_string();
        let device_updated = if let Some(device) = self.config.ui.device_renames.get_mut(&path_str)
        {
            device.name = new_name.clone();
            true
        } else {
            let path_trimmed = path_str.trim_end_matches('/').to_string();
            if let Some(device) = self.config.ui.device_renames.get_mut(&path_trimmed) {
                device.name = new_name.clone();
                true
            } else {
                self.config.ui.device_renames.insert(
                    path_str.clone(),
                    crate::model::DeviceRename {
                        name: new_name.clone(),
                        icon: None,
                    },
                );
                true
            }
        };

        modified |= device_updated;

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

    pub fn handle_add_tag_to_sidebar(&mut self, tag: String) {
        let uri = format!("tags://{}", tag.trim_start_matches('#'));
        self.config.sidebar.push(crate::model::CustomPlace {
            name: format!("# {}", tag.trim_start_matches('#')),
            kind: None,
            icon: "tag-symbolic".to_string(),
            path: uri,
        });
        crate::utils::save_config(&self.config);
        self.refresh_sidebar();
    }
}
