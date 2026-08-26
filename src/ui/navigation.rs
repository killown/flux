use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Handles backward navigation in the directory history stack.
    pub fn handle_go_back(&mut self, sender: &AsyncComponentSender<Self>) {
        self.reset_from_content_search();
        if let Some(prev) = self.history.pop() {
            self.forward_stack.push(self.current_path.clone());
            if crate::services::network::is_network_uri(&prev) {
                self.current_path = prev.clone();
                self.load_network(&prev.to_string_lossy(), None, sender.clone());
            } else {
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
                self.load_network(&parent_path.to_string_lossy(), None, sender.clone());
            } else {
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
        if let Some(next) = self.forward_stack.pop() {
            self.history.push(self.current_path.clone());
            if self.history.len() > constants::MAX_HISTORY {
                self.history.remove(0);
            }
            if crate::services::network::is_network_uri(&next) {
                self.current_path = next.clone();
                self.load_network(&next.to_string_lossy(), None, sender.clone());
            } else {
                self.load_path(next, sender);
            }
            self.update_breadcrumbs();
            if let Some(entry) = self.header_path_entry.upgrade() {
                entry.set_text(&self.current_path.to_string_lossy());
                entry.set_position(entry.text_length() as i32);
            }
        }
    }

    //WARN: Change this logic with caution.
    // If the process working directory
    // (CWD) is not synchronized, operations like drag-and-drop or shell commands
    // may resolve relative paths incorrectly, moving files to previous locations
    // instead of the directory currently displayed to the user.
    pub fn handle_navigate(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        let path_str = path.to_string_lossy();

        // 0. Intercept Tag searches typed into location bar or clicked in sidebar
        if path_str == "tags://" {
            sender.input(AppMsg::OpenTagNavigator);
            return;
        }

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

            sender.input(AppMsg::NavigateTag(clean_tag.to_string()));
            return;
        }

        // 1. Intercept Network URIs
        if crate::services::network::is_network_uri(&path) {
            if path == self.current_path {
                return;
            }

            let old_path = std::mem::replace(&mut self.current_path, path.clone());

            self.recent_stack.retain(|p| p != &path && p != &old_path);
            self.recent_stack.push_front(old_path.clone());
            self.recent_stack.truncate(constants::MAX_RECENT_ITEMS);

            self.filter.clear();
            self.files.clear_filters();
            sender.input(AppMsg::CloseSearchSync);

            if self.header_view == constants::VIEW_SEARCH {
                self.header_view = "path".to_string();
            }

            self.history.push(old_path);
            if self.history.len() > constants::MAX_HISTORY {
                self.history.remove(0);
            }
            self.forward_stack.clear();
            self.forward_stack.clear();

            self.load_network(&path_str, None, sender.clone());
            self.update_breadcrumbs();
            if let Some(entry) = self.header_path_entry.upgrade() {
                entry.set_text(&self.current_path.to_string_lossy());
                entry.set_position(entry.text_length() as i32);
            }
            return;
        }

        // 2. Intercept Virtual Archive URIs
        if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
            if let Some((archive_path, prefix)) =
                crate::services::archive::parse_archive_uri(&path_str)
            {
                if path == self.current_path {
                    return;
                }

                let old_path = std::mem::replace(&mut self.current_path, path.clone());

                self.recent_stack.retain(|p| p != &path && p != &old_path);
                self.recent_stack.push_front(old_path.clone());
                self.recent_stack.truncate(constants::MAX_RECENT_ITEMS);

                self.filter.clear();
                self.files.clear_filters();
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

                let view = self.files.view.clone();
                glib::idle_add_local_once(move || {
                    view.grab_focus();
                });
            }
            return;
        }

        // 3. Local filesystem validation
        let path_valid = path_str == "/"
            || path.exists()
            || path_str.starts_with(constants::TRASH_URI)
            || path_str.starts_with(constants::RECENT_URI);

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

        if path == self.current_path {
            return;
        }

        if path.is_dir()
            || path_str.starts_with(constants::TRASH_URI)
            || path_str.starts_with(constants::RECENT_URI)
        {
            self.archive_locked = false;
            let old_path = std::mem::replace(&mut self.current_path, path.clone());

            if path.is_absolute() {
                let _ = std::env::set_current_dir(&path);
            }

            self.recent_stack.retain(|p| p != &path && p != &old_path);
            self.recent_stack.push_front(old_path.clone());
            self.recent_stack.truncate(constants::MAX_RECENT_ITEMS);

            self.filter.clear();
            self.files.clear_filters();

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

            let view = self.files.view.clone();
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
        let old_path = std::mem::replace(
            &mut self.current_path,
            crate::services::archive::build_archive_uri(&archive_path, ""),
        );

        self.recent_stack
            .retain(|p| p != &self.current_path && p != &old_path);
        self.recent_stack.push_front(old_path.clone());
        self.recent_stack.truncate(constants::MAX_RECENT_ITEMS);

        self.filter.clear();
        self.files.clear_filters();
        sender.input(AppMsg::CloseSearchSync);

        if self.header_view == constants::VIEW_SEARCH {
            self.header_view = constants::VIEW_PATH.to_string();
        }

        self.history.push(old_path);
        self.forward_stack.clear();

        self.load_archive(archive_path, String::new(), None, sender);
        self.update_breadcrumbs();
        if let Some(entry) = self.header_path_entry.upgrade() {
            entry.set_text(&self.current_path.to_string_lossy());
            entry.set_position(entry.text_length() as i32);
        }

        let view = self.files.view.clone();
        glib::idle_add_local_once(move || {
            view.grab_focus();
        });
    }

    /// Adds a path or multiple selected paths to the temporary quick panel list.
    pub fn handle_add_exclusive(
        &mut self,
        explicit_path: Option<PathBuf>,
        sender: &AsyncComponentSender<Self>,
    ) {
        let mut paths_to_add = Vec::new();

        if let Some(path) = explicit_path {
            paths_to_add.push(path);
        } else {
            let selection = self.get_selection();
            if selection.is_empty() {
                paths_to_add.push(self.current_path.clone());
            } else {
                paths_to_add.extend(selection);
            }
        }

        let mut added_any = false;
        for path in paths_to_add {
            let canon = path.canonicalize().unwrap_or(path);
            if !self.exclusive_list.contains(&canon) {
                self.exclusive_list.push(canon);
                added_any = true;
            }
        }

        if added_any {
            if self.exclusive_index.is_none() && !self.exclusive_list.is_empty() {
                self.exclusive_index = Some(0);
            }
            sender.input(AppMsg::RebuildQuickPanel);
        }
    }
    /// Clears all entries from the temporary quick panel list.
    pub fn handle_clear_exclusive(&mut self, sender: &AsyncComponentSender<Self>) {
        self.exclusive_list.clear();
        self.exclusive_index = None;
        sender.input(AppMsg::RebuildQuickPanel);
    }

    /// Removes a specific path from the quick panel list.
    pub fn handle_remove_quick_item(&mut self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        if let Some(pos) = self.exclusive_list.iter().position(|p| p == &path) {
            self.exclusive_list.remove(pos);
            self.exclusive_index = if self.exclusive_list.is_empty() {
                None
            } else {
                Some(pos.saturating_sub(1).min(self.exclusive_list.len() - 1))
            };
            sender.input(AppMsg::RebuildQuickPanel);
        }
    }

    /// Reconstructs the quick panel button bar widget layout.
    pub fn handle_rebuild_quick_panel(&self, sender: &AsyncComponentSender<Self>) {
        let panel = &self.quick_panel_box;
        while let Some(child) = panel.first_child() {
            panel.remove(&child);
        }
        let active_idx = self.exclusive_index;
        for (idx, path) in self.exclusive_list.iter().enumerate() {
            let label = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());

            let btn = gtk::Button::with_label(&label);
            btn.set_tooltip_text(Some(&path.to_string_lossy()));
            btn.add_css_class("flat");
            if active_idx == Some(idx) {
                btn.add_css_class("suggested-action");
            }

            let path_nav = path.clone();
            let s_nav = sender.clone();
            btn.connect_clicked(move |_| {
                s_nav.input(AppMsg::Navigate(path_nav.clone()));
            });

            let path_rm = path.clone();
            let s_rm = sender.clone();
            let middle = gtk::GestureClick::new();
            middle.set_button(2);
            middle.connect_pressed(move |gesture, _, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                s_rm.input(AppMsg::RemoveQuickItem(path_rm.clone()));
            });
            btn.add_controller(middle);

            // ── Add Drop Target for quick-list button ──
            let formats = gtk::gdk::ContentFormats::builder()
                .add_type(gtk::gdk::FileList::static_type())
                .build();

            let drop_target = gtk::DropTarget::builder()
                .actions(gtk::gdk::DragAction::MOVE | gtk::gdk::DragAction::COPY)
                .formats(&formats)
                .build();

            let target_path = path.clone();
            let sender_drop = sender.clone();

            drop_target.connect_drop(move |_target, value, _, _| {
                if let Ok(file_list) = value.get::<gtk::gdk::FileList>() {
                    let source_paths: Vec<PathBuf> = file_list
                        .files()
                        .into_iter()
                        .filter_map(|f| f.path())
                        .collect();

                    if !source_paths.is_empty() {
                        sender_drop.input(AppMsg::MoveFilesToTarget {
                            sources: source_paths,
                            destination: target_path.clone(),
                        });
                        return true;
                    }
                }
                false
            });

            btn.add_controller(drop_target);
            panel.append(&btn);
        }
    }

    /// Switches to the next item in the quick panel list.
    pub fn handle_next_exclusive(&mut self, sender: &AsyncComponentSender<Self>) {
        if !self.exclusive_list.is_empty() {
            let new_idx = match self.exclusive_index {
                Some(i) => (i + 1) % self.exclusive_list.len(),
                None => 0,
            };
            self.exclusive_index = Some(new_idx);
            let target = self.exclusive_list[new_idx].clone();
            sender.input(AppMsg::Navigate(target));
            sender.input(AppMsg::RebuildQuickPanel);
        }
    }

    /// Switches to the previous item in the quick panel list.
    pub fn handle_prev_exclusive(&mut self, sender: &AsyncComponentSender<Self>) {
        if !self.exclusive_list.is_empty() {
            let new_idx = match self.exclusive_index {
                Some(i) if i > 0 => i - 1,
                _ => self.exclusive_list.len() - 1,
            };
            self.exclusive_index = Some(new_idx);
            let target = self.exclusive_list[new_idx].clone();
            sender.input(AppMsg::Navigate(target));
            sender.input(AppMsg::RebuildQuickPanel);
        }
    }
}
