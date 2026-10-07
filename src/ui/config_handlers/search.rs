use crate::model::FluxApp;
use crate::utils;
use relm4::prelude::*;

impl FluxApp {
    pub fn handle_set_search_panel_width(&mut self, val: i32) {
        let clamped = val.clamp(200, 700);
        self.config.ui.search_panel_width = clamped;
        utils::save_config(&self.config);
    }

    pub fn handle_set_tag_panel_width(&mut self, val: i32) {
        let clamped = val.clamp(250, 800);
        self.config.ui.tag_panel_width = clamped;
        utils::save_config(&self.config);
    }

    pub fn handle_set_max_content_search_results(
        &mut self,
        val: usize,
        _sender: &AsyncComponentSender<Self>,
    ) {
        self.config.ui.max_content_search_results = val;
        utils::save_config(&self.config);
    }

    pub fn handle_set_max_search_results(&mut self, val: usize) {
        self.config.ui.max_search_results = val;
        utils::save_config(&self.config);
    }
}
