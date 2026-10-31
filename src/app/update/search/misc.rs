use crate::app::state::FluxApp;
use crate::utils;

impl FluxApp {
    pub fn handle_set_disable_drag_and_drop(&mut self, val: bool) {
        crate::hit!("handle_set_disable_drag_and_drop");
        self.config.ui.disable_drag_and_drop = val;
        utils::save_config(&self.config);
    }
}
