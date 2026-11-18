use crate::app::state::FluxApp;
use crate::utils;
use adw::prelude::*;

impl FluxApp {
    pub fn handle_set_maximized(&mut self, max: bool) {
        crate::hit!("handle_set_maximized");
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
        crate::hit!("handle_set_window_size");
        if let Some(w) = width {
            self.config.ui.startup_window_width = w;
        }
        if let Some(h) = height {
            self.config.ui.startup_window_height = h;
        }
        utils::save_config(&self.config);
    }

    /// Records which parts the resize guard has hidden so the view keeps them hidden.
    pub fn handle_set_auto_hidden(&mut self, sidebar: bool, header: bool, statusbar: bool) {
        crate::hit!("handle_set_auto_hidden");
        self.auto_hidden_sidebar = sidebar;
        self.auto_hidden_header = header;
        self.auto_hidden_statusbar = statusbar;
    }
}
