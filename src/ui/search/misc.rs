use crate::model::FluxApp;
use crate::utils;

impl FluxApp {
    pub fn handle_set_disable_drag_and_drop(&mut self, val: bool) {
        self.config.ui.disable_drag_and_drop = val;
        utils::save_config(&self.config);
    }
}
