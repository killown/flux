use crate::model::AltCycle;
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
        crate::hit!("handle_navigate");
        let path_str = path.to_string_lossy();
        self.mime_filter = None;
        self.mime_filter_from_space = false;

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

        // Intercept Contextual Actions panel shortcut entry
        if path_str == "actions://" || path_str == "actions:///" {
            sender.input(AppMsg::ToggleActionsPanel);
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
        self.navigate_owe_first_selection();

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
        }
    }

    /// Enters a compressed archive as a virtual browsing location.
    pub fn handle_enter_archive(
        &mut self,
        archive_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_enter_archive");
        self.mime_filter = None;
        self.reset_from_content_search();
        self.last_search_was_advanced = false;
        self.navigate_owe_first_selection();
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
    }

    /// Jump to the next sibling folder whose name starts with `letter`.
    /// Also driven by plain letters while Navigate mode is on.
    ///
    /// Repeated presses of the same chord cycle through the matching
    /// folders in the current sort order. If there is no current cycle
    /// for this `(directory, letter)`, starts at the first match.
    /// Does nothing when no folder matches.
    pub fn handle_cycle_by_letter(&mut self, letter: char, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_cycle_by_letter");
        let prefix = letter.to_ascii_lowercase();
        let current = self.current_path.clone();

        // Is `current` inside an active cycle for this letter?
        // Either we are exactly at the anchor, or we are inside one of
        // the recorded candidates (anchor/<candidate>).
        let existing = self.alt_cycle.take().and_then(|c| {
            if c.letter != prefix {
                return None;
            }
            let inside_candidate = c.candidates.iter().any(|p| Self::cycle_inside(&current, p));
            if current == c.anchor || inside_candidate {
                Some(c)
            } else {
                None
            }
        });

        let cycle = match existing {
            Some(mut c) => {
                c.index = (c.index + 1) % c.candidates.len();
                c
            }
            None => {
                // First press: build the candidate list from the current
                // directory's loaded FileItems.
                let found: Vec<(PathBuf, bool)> = {
                    let files = &self.tabs[self.active_tab_index].files;
                    (0..files.len())
                        .filter_map(|i| files.get(i))
                        .filter_map(|w| {
                            let it = w.borrow();
                            if it.is_folder_like()
                                && it.name.to_lowercase().starts_with(prefix)
                                && it.path.parent() == Some(current.as_path())
                            {
                                Some((it.path.clone(), it.is_virtual_dir))
                            } else {
                                None
                            }
                        })
                        .collect()
                };
                if found.is_empty() {
                    return;
                }
                let virtual_targets = found
                    .iter()
                    .filter(|(_, is_virtual)| *is_virtual)
                    .map(|(path, _)| path.clone())
                    .collect();
                AltCycle {
                    anchor: current,
                    letter: prefix,
                    candidates: found.into_iter().map(|(path, _)| path).collect(),
                    index: 0,
                    virtual_targets,
                }
            }
        };

        let target = cycle.candidates[cycle.index].clone();
        let is_virtual = cycle.virtual_targets.contains(&target);
        self.alt_cycle = Some(cycle);
        // An archive drawn as a folder is entered as an archive, not navigated to.
        if is_virtual {
            sender.input(AppMsg::EnterArchive(target));
        } else {
            sender.input(AppMsg::Navigate(target));
        }
    }

    /// True when `current` is `candidate` itself, below it, or inside it when
    /// `candidate` is an archive being browsed.
    pub(super) fn cycle_inside(current: &std::path::Path, candidate: &std::path::Path) -> bool {
        current == candidate
            || current.starts_with(candidate)
            || crate::services::archive::parse_archive_uri(&current.to_string_lossy())
                .is_some_and(|(archive, _)| archive == candidate)
    }

    /// Cancel the cycle. If we descended into a candidate, go back to the
    /// anchor so the next letter press starts from scratch.
    pub fn handle_cycle_by_letter_cancel(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_cycle_by_letter_cancel");
        let Some(cycle) = self.alt_cycle.take() else {
            return;
        };
        let inside_candidate = cycle
            .candidates
            .iter()
            .any(|p| Self::cycle_inside(&self.current_path, p));
        if inside_candidate {
            sender.input(AppMsg::Navigate(cycle.anchor));
        }
        sender.input(AppMsg::ShowToast(crate::i18n::tr("Cycle cancelled")));
    }

    /// Forget the cycle without navigating. The next letter press builds
    /// candidates from `current_path`.
    pub fn handle_cycle_by_letter_reanchor(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_cycle_by_letter_reanchor");
        self.alt_cycle = None;
        sender.input(AppMsg::ShowToast(crate::i18n::tr(
            "Cycle anchored to current folder",
        )));
    }
}
