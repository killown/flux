use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    //WARN: Change this logic with caution.
    // If the process working directory
    // (CWD) is not synchronized, operations like drag-and-drop or shell commands
    // may resolve relative paths incorrectly, moving files to previous locations
    // instead of the directory currently displayed to the user.
    pub fn handle_navigate(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        let path_str = path.to_string_lossy();

        // Intercept panel toggles before touching any current state or running resets
        if path_str == "tags://" || path_str == "tags:///" {
            sender.input(AppMsg::ToggleTagPanel);
            return;
        }

        // Intercept Search panel shortcut URI
        if path_str == "search://" || path_str == "search:///" {
            sender.input(AppMsg::ToggleSearchPanel);
            return;
        }

        if self.diff_panel_visible {
            self.toggle_sidebar_right_panel(crate::model::RightPanelType::Diff, sender);
        }

        let was_in_archive = self
            .current_path
            .to_string_lossy()
            .starts_with(crate::services::archive::ARCHIVE_URI);
        let will_be_in_archive = path_str.starts_with(crate::services::archive::ARCHIVE_URI);

        if was_in_archive && !will_be_in_archive {
            // Defer so xdg-open / GtkVideo have time to open the file.
            // Linux unlink keeps the file alive while any fd holds it, but the
            // external app may not have called open() yet.
            glib::timeout_add_local_once(std::time::Duration::from_secs(3), || {
                crate::services::archive::clear_archive_session_temp();
            });
        }

        // Guard against re-navigating to the current folder (by string or canonical target)
        // WARNING: Do not remove or modify this check without careful consideration.
        // Removing this check causes redundant navigation to the currently active path,
        // clearing the file grid and causing all items to disappear on repeated navigation.
        let has_filter = !self.filter.is_empty();

        if !has_filter && path == self.current_path {
            return;
        }

        if !has_filter
            && !crate::services::network::is_network_uri(&path)
            && !path_str.starts_with(crate::services::archive::ARCHIVE_URI)
            && !path_str.starts_with(constants::TRASH_URI)
            && !path_str.starts_with(constants::RECENT_URI)
        {
            if let (Ok(p1), Ok(p2)) = (path.canonicalize(), self.current_path.canonicalize()) {
                if p1 == p2 {
                    return;
                }
            }
        }

        self.reset_from_content_search();
        self.last_search_was_advanced = false;

        //--------------------------------------------------------------------------------------//
        //NOTE: this block of code is where in can intercept and handle special URIs or commands before
        // proceeding with normal navigation.

        //--------------------------------------------------------------------------------------//

        if path_str.starts_with('#')
            || path_str.starts_with(":tag:")
            || path_str.starts_with(":t:")
            || path_str.starts_with("tags://")
        {
            let clean_tag = path_str
                .trim_start_matches("tags://")
                .trim_start_matches(":tag:")
                .trim_start_matches(":t:")
                .trim_start_matches('#');

            if clean_tag.is_empty() {
                sender.input(AppMsg::ToggleTagPanel);
            } else {
                sender.input(AppMsg::SwitchHeader(constants::VIEW_SEARCH.to_string()));
                sender.input(AppMsg::UpdateFilter(format!("#{}", clean_tag)));

                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    let title = format!("#{}", clean_tag);
                    tab.title = title.clone();
                    if (self.active_tab_index as i32) < self.tab_view.n_pages() {
                        let page = self.tab_view.nth_page(self.active_tab_index as i32);
                        page.set_title(&title);
                    }
                }
            }
            return;
        }

        // Intercept Network URIs
        if crate::services::network::is_network_uri(&path) {
            let old_path = std::mem::replace(&mut self.current_path, path.clone());

            self.sync_sidebar_selection();

            self.recent_stack.retain(|p| p != &path && p != &old_path);
            self.recent_stack.push_front(old_path.clone());
            self.recent_stack.truncate(constants::MAX_RECENT_ITEMS);

            self.filter.clear();
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                tab.files.clear_filters();
            }
            sender.input(AppMsg::CloseSearchSync);

            if self.header_view == constants::VIEW_SEARCH {
                self.header_view = "path".to_string();
            }

            self.history.push(old_path);
            if self.history.len() > constants::MAX_HISTORY {
                self.history.remove(0);
            }
            self.forward_stack.clear();

            self.load_network(&path_str, None, sender.clone());
            self.update_breadcrumbs();
            if let Some(entry) = self.header_path_entry.upgrade() {
                entry.set_text(&self.current_path.to_string_lossy());
                entry.set_position(entry.text_length() as i32);
            }

            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                tab.current_path = self.current_path.clone();
                tab.history = self.history.clone();
                tab.forward_stack = self.forward_stack.clone();
                tab.scroll_offset = 0.0;
                let title = self
                    .current_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| self.current_path.to_string_lossy().to_string());
                tab.title = title.clone();
                if (self.active_tab_index as i32) < self.tab_view.n_pages() {
                    let page = self.tab_view.nth_page(self.active_tab_index as i32);
                    page.set_title(&title);
                }
            }

            return;
        }

        // Intercept Virtual Archive URIs
        if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
            if let Some((archive_path, prefix)) =
                crate::services::archive::parse_archive_uri(&path_str)
            {
                let old_path = std::mem::replace(&mut self.current_path, path.clone());

                self.sync_sidebar_selection();

                self.recent_stack.retain(|p| p != &path && p != &old_path);
                self.recent_stack.push_front(old_path.clone());
                self.recent_stack.truncate(constants::MAX_RECENT_ITEMS);

                self.filter.clear();
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    tab.files.clear_filters();
                }
                sender.input(AppMsg::CloseSearchSync);

                if self.header_view == constants::VIEW_SEARCH {
                    self.header_view = "path".to_string();
                }

                self.history.push(old_path);
                self.forward_stack.clear();

                self.load_archive(archive_path, prefix, None, sender);
                self.update_breadcrumbs();
                if let Some(entry) = self.header_path_entry.upgrade() {
                    entry.set_text(&self.current_path.to_string_lossy());
                    entry.set_position(entry.text_length() as i32);
                }

                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    tab.current_path = self.current_path.clone();
                    tab.history = self.history.clone();
                    tab.forward_stack = self.forward_stack.clone();
                    tab.scroll_offset = 0.0;
                    let title = self
                        .current_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| self.current_path.to_string_lossy().to_string());
                    tab.title = title.clone();
                    if (self.active_tab_index as i32) < self.tab_view.n_pages() {
                        let page = self.tab_view.nth_page(self.active_tab_index as i32);
                        page.set_title(&title);
                    }
                }

                let view = self.tabs[self.active_tab_index].files.view.clone();
                glib::idle_add_local_once(move || {
                    view.grab_focus();
                });
            }
            return;
        }

        // Local filesystem validation
        let path_valid = path_str == "/"
            || path.exists()
            || path_str.starts_with(constants::TRASH_URI)
            || path_str.starts_with(constants::RECENT_URI)
            || path_str.starts_with("tags://");

        if !path_valid {
            sender.input(AppMsg::ShowToast(format!(
                "{}: {}",
                crate::i18n::tr("Path not found"),
                path_str
            )));
            return;
        }

        if let Some(pos) = self.exclusive_list.iter().position(|p| p == &path) {
            self.exclusive_index = Some(pos);
            sender.input(AppMsg::RebuildQuickPanel);
        }

        if path.is_dir()
            || path_str.starts_with(constants::TRASH_URI)
            || path_str.starts_with(constants::RECENT_URI)
        {
            self.archive_locked = false;
            let old_path = std::mem::replace(&mut self.current_path, path.clone());

            self.sync_sidebar_selection();

            if path.is_absolute() {
                let _ = std::env::set_current_dir(&path);
            }

            self.recent_stack.retain(|p| p != &path && p != &old_path);
            self.recent_stack.push_front(old_path.clone());
            self.recent_stack.truncate(constants::MAX_RECENT_ITEMS);

            self.filter.clear();
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                tab.files.clear_filters();
            }

            sender.input(AppMsg::CloseSearchSync);

            if self.header_view == constants::VIEW_SEARCH {
                self.header_view = constants::VIEW_PATH.to_string();
            }

            self.history.push(old_path);
            self.forward_stack.clear();

            self.load_path(path, sender);
            self.update_breadcrumbs();
            if let Some(entry) = self.header_path_entry.upgrade() {
                entry.set_text(&self.current_path.to_string_lossy());
                entry.set_position(entry.text_length() as i32);
            }

            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                tab.current_path = self.current_path.clone();
                tab.history = self.history.clone();
                tab.forward_stack = self.forward_stack.clone();
                tab.scroll_offset = 0.0;
                let title = self
                    .current_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| self.current_path.to_string_lossy().to_string());
                tab.title = title.clone();
                if (self.active_tab_index as i32) < self.tab_view.n_pages() {
                    let page = self.tab_view.nth_page(self.active_tab_index as i32);
                    page.set_title(&title);
                }
            }

            let view = self.tabs[self.active_tab_index].files.view.clone();
            let terminal = self.terminal.clone();
            let terminal_visible = self.terminal_visible;
            glib::idle_add_local_once(move || {
                let terminal_has_focus = terminal_visible && terminal.has_focus();
                if !terminal_has_focus {
                    view.grab_focus();
                }
            });
        }
    }

    /// Enters a compressed archive as a virtual browsing location.
    pub fn handle_enter_archive(
        &mut self,
        archive_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.reset_from_content_search();
        self.last_search_was_advanced = false;
        let old_path = std::mem::replace(
            &mut self.current_path,
            crate::services::archive::build_archive_uri(&archive_path, ""),
        );

        self.sync_sidebar_selection();

        self.recent_stack
            .retain(|p| p != &self.current_path && p != &old_path);
        self.recent_stack.push_front(old_path.clone());
        self.recent_stack.truncate(constants::MAX_RECENT_ITEMS);

        self.filter.clear();
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.files.clear_filters();
        }
        sender.input(AppMsg::CloseSearchSync);

        if self.header_view == constants::VIEW_SEARCH {
            self.header_view = constants::VIEW_PATH.to_string();
        }

        self.history.push(old_path);
        self.forward_stack.clear();

        // Synchronize active tab navigation and header page title
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.current_path = self.current_path.clone();
            tab.history = self.history.clone();
            tab.forward_stack = self.forward_stack.clone();
            tab.recent_stack = self.recent_stack.clone();
            tab.scroll_offset = 0.0;
            let title = archive_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| self.current_path.to_string_lossy().to_string());
            tab.title = title.clone();
            if (self.active_tab_index as i32) < self.tab_view.n_pages() {
                let page = self.tab_view.nth_page(self.active_tab_index as i32);
                page.set_title(&title);
            }
        }

        self.load_archive(archive_path, String::new(), None, sender);
        self.update_breadcrumbs();
        if let Some(entry) = self.header_path_entry.upgrade() {
            entry.set_text(&self.current_path.to_string_lossy());
            entry.set_position(entry.text_length() as i32);
        }

        let view = self.tabs[self.active_tab_index].files.view.clone();
        glib::idle_add_local_once(move || {
            view.grab_focus();
        });
    }
}
