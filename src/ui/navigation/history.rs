use crate::model::FluxApp;
use crate::ui::constants;
use gtk::prelude::*;
use relm4::prelude::*;

impl FluxApp {
    /// Handles backward navigation in the directory history stack.
    pub fn handle_go_back(&mut self, sender: &AsyncComponentSender<Self>) {
        self.reset_from_content_search();
        self.last_search_was_advanced = false;
        if self.diff_panel_visible {
            self.toggle_sidebar_right_panel(crate::model::RightPanelType::Diff, sender);
        }
        if let Some(prev) = self.history.pop() {
            self.forward_stack.push(self.current_path.clone());
            if crate::services::network::is_network_uri(&prev) {
                self.current_path = prev.clone();
                self.sync_sidebar_selection();
                self.load_network(&prev.to_string_lossy(), None, sender.clone());
            } else {
                self.current_path = prev.clone();
                self.sync_sidebar_selection();
                self.load_path(prev, sender);
            }
            self.update_breadcrumbs();
            if let Some(entry) = self.header_path_entry.upgrade() {
                entry.set_text(&self.current_path.to_string_lossy());
                entry.set_position(entry.text_length() as i32);
            }
        } else if let Some(parent) = self.current_path.parent() {
            let parent_path = parent.to_path_buf();
            self.forward_stack.push(self.current_path.clone());
            if crate::services::network::is_network_uri(&parent_path) {
                self.current_path = parent_path.clone();
                self.sync_sidebar_selection();
                self.load_network(&parent_path.to_string_lossy(), None, sender.clone());
            } else {
                self.current_path = parent_path.clone();
                self.sync_sidebar_selection();
                self.load_path(parent_path, sender);
            }
            self.update_breadcrumbs();
            if let Some(entry) = self.header_path_entry.upgrade() {
                entry.set_text(&self.current_path.to_string_lossy());
                entry.set_position(entry.text_length() as i32);
            }
        }
    }

    /// Handles forward navigation in the directory history stack.
    pub fn handle_go_forward(&mut self, sender: &AsyncComponentSender<Self>) {
        self.reset_from_content_search();
        self.last_search_was_advanced = false;
        if self.diff_panel_visible {
            self.toggle_sidebar_right_panel(crate::model::RightPanelType::Diff, sender);
        }
        while let Some(next) = self.forward_stack.pop() {
            let s = next.to_string_lossy();
            let is_valid = s == "/"
                || next.exists()
                || s.starts_with("trash:///")
                || s.starts_with("recent:///")
                || crate::services::network::is_network_uri(&next)
                || s.starts_with(crate::services::archive::ARCHIVE_URI);

            if !is_valid {
                continue;
            }

            self.history.push(self.current_path.clone());
            if self.history.len() > constants::MAX_HISTORY {
                self.history.remove(0);
            }

            if crate::services::network::is_network_uri(&next) {
                self.current_path = next.clone();
                self.sync_sidebar_selection();
                self.load_network(&next.to_string_lossy(), None, sender.clone());
            } else {
                self.current_path = next.clone();
                self.sync_sidebar_selection();
                self.load_path(next, sender);
            }

            self.update_breadcrumbs();
            if let Some(entry) = self.header_path_entry.upgrade() {
                entry.set_text(&self.current_path.to_string_lossy());
                entry.set_position(entry.text_length() as i32);
            }
            break;
        }
    }
}
