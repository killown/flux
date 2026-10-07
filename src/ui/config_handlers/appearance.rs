use crate::model::{AppMsg, BackgroundSlot, FluxApp};
use crate::utils;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn handle_set_ui_scale(&mut self, scale: f64) {
        self.config.ui.ui_scale = scale;
        utils::save_config(&self.config);
        crate::utils::helpers::apply_ui_scale(scale);
    }

    pub fn handle_set_background_alpha(&mut self, slot: crate::model::BackgroundSlot, val: f64) {
        let clamped = val.clamp(0.0, 1.0);
        match slot {
            crate::model::BackgroundSlot::Window => self.config.ui.bg_alpha_window = clamped,
            crate::model::BackgroundSlot::SidebarLeft => {
                self.config.ui.bg_alpha_sidebar_left = clamped
            }
            crate::model::BackgroundSlot::SidebarRight => {
                self.config.ui.bg_alpha_sidebar_right = clamped
            }
        }
        crate::utils::save_config(&self.config);
        crate::utils::helpers::load_custom_background_images();
        if let Some(ref w) = self.sidebar_widget {
            w.queue_draw();
        }
        self.files.view.queue_draw();
    }

    pub fn handle_set_flux_background(&mut self, target: PathBuf, slot: BackgroundSlot) {
        if let Some(data_dir) = dirs::data_dir() {
            let img_dir = data_dir.join("flux/data/resources/images");
            let _ = std::fs::create_dir_all(&img_dir);

            let prefix = match slot {
                BackgroundSlot::Window => "window",
                BackgroundSlot::SidebarLeft => "sidebar-left",
                BackgroundSlot::SidebarRight => "sidebar-right",
            };

            // Remove any previous images for this slot
            if let Ok(entries) = std::fs::read_dir(&img_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        if stem == prefix || stem.starts_with(&format!("{}_", prefix)) {
                            let _ = std::fs::remove_file(p);
                        }
                    }
                }
            }

            // Save with a unique timestamp to invalidate GTK's CSS URL cache
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let dest = img_dir.join(format!("{}_{}.png", prefix, ts));

            if std::fs::copy(&target, &dest).is_ok() {
                crate::utils::helpers::load_custom_background_images();
                if let Some(ref w) = self.sidebar_widget {
                    w.queue_draw();
                }
                self.tabs[self.active_tab_index].files.view.queue_draw();
            }
        }
    }

    pub fn handle_clear_flux_backgrounds(&self, sender: &AsyncComponentSender<Self>) {
        crate::utils::helpers::clear_custom_background_images();
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_toggle_header_bar(&mut self) {
        self.header_visible = !self.header_visible;
        self.config.ui.header_visible = self.header_visible;
        utils::save_config(&self.config);

        if let Some(ref widget) = self.header_widget {
            widget.set_visible(self.header_visible);
        }
    }

    pub fn handle_set_show_csd(&mut self, val: bool) {
        self.config.ui.show_csd = val;
        crate::utils::save_config(&self.config);
    }

    pub fn handle_set_window_controls_left(&mut self, val: bool) {
        self.config.ui.window_controls_left = val;
        crate::utils::save_config(&self.config);
    }

    pub fn handle_set_theme(&mut self, theme: Option<String>) {
        self.config.ui.theme = theme;
        utils::save_config(&self.config);
        crate::utils::helpers::load_custom_css();
        self.terminal.apply_theme(&self.config.ui.terminal);
    }

    pub fn handle_set_diff_panel_width(&mut self, val: i32) {
        let clamped = val.clamp(280, 900);
        self.config.ui.diff_panel_width = clamped;
        utils::save_config(&self.config);
    }

    pub fn handle_set_auto_show_diff(&mut self, val: bool) {
        self.config.ui.auto_show_diff = val;
        utils::save_config(&self.config);
    }
}
