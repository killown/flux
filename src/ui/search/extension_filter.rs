use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

impl FluxApp {
    #[allow(dead_code)]
    pub fn handle_set_extension_filter(
        &mut self,
        patterns: Vec<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        let expanded: Vec<String> = patterns
            .iter()
            .flat_map(|p| crate::utils::glob::expand_mime_category(p))
            .collect();
        self.extension_filter = if expanded.is_empty() {
            None
        } else {
            Some(expanded.clone())
        };
        self.extension_globset = crate::utils::glob::compile_patterns(&expanded);
        sender.input(AppMsg::Refresh);
    }

    #[allow(dead_code)]
    pub fn handle_clear_extension_filter(&mut self, sender: &AsyncComponentSender<Self>) {
        self.extension_filter = None;
        self.extension_globset = None;
        sender.input(AppMsg::Refresh);
    }
}
