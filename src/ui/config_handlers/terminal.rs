use crate::model::FluxApp;
use crate::utils;

impl FluxApp {
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
