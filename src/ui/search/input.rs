use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use relm4::prelude::*;

impl FluxApp {
    /// Handles single character keystroke capture for type-to-search.
    pub fn handle_search_input(&mut self, c: char) {
        self.search_just_opened = true;
        self.filter.push(c);
        self.header_view = constants::VIEW_SEARCH.to_string();
    }

    /// Handles backspace press in search entry mode.
    pub fn handle_search_backspace(&mut self, sender: &AsyncComponentSender<Self>) {
        if !self.filter.is_empty() {
            self.filter.pop();
            let query = self.filter.clone();
            sender.input(AppMsg::UpdateFilter(query));
        }
    }

    /// Handles header view stack switches (e.g. search <-> entry <-> path).
    pub fn handle_switch_header(&mut self, view_name: String) {
        if self.header_view == constants::VIEW_SEARCH
            && view_name != constants::VIEW_SEARCH
            && self.is_content_searching
        {
            self.reset_from_content_search();
        } else if self.header_view == constants::VIEW_SEARCH
            && view_name != constants::VIEW_SEARCH
            && !self.is_content_searching
            && self.is_list_mode != self.saved_list_mode
        {
            self.is_list_mode = self.saved_list_mode;
            self.tabs[self.active_tab_index]
                .files
                .view
                .set_max_columns(self.saved_max_columns);
            self.sync_list_mode();
            self.search_saved_layout = false;
        }
        self.header_view = view_name;
        if self.header_view != constants::VIEW_SEARCH {
            self.filter.clear();
            self.search_just_opened = true;
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                tab.files.clear_filters();
            }
        }
    }
}
