use crate::domain::model::{AppMsg, FluxApp, NavigationMsg};
use relm4::prelude::*;

impl FluxApp {
    #[allow(dead_code)]
    pub fn handle_set_extension_filter(
        &mut self,
        patterns: Vec<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_set_extension_filter");
        let expanded: Vec<String> = patterns
            .iter()
            .flat_map(|p| crate::utils::glob::expand_mime_category(p))
            .collect();
        self.filters.extension = if expanded.is_empty() {
            None
        } else {
            Some(expanded.clone())
        };
        self.filters.extension_globset = crate::utils::glob::compile_patterns(&expanded);
        sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
    }

    #[allow(dead_code)]
    pub fn handle_clear_extension_filter(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_clear_extension_filter");
        self.filters.extension = None;
        self.filters.extension_globset = None;
        sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
    }
}
