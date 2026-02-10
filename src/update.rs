use crate::file_properties::FileProperties;
use crate::model::{AppMsg, FluxApp, SortBy};
use crate::utils;
use adw::gdk;
use adw::prelude::*;
use gtk::{gio, glib};
use relm4::prelude::*;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn handle_update(&mut self, message: AppMsg, sender: ComponentSender<Self>) {
        match message {
            AppMsg::RefreshSidebar => {
                self.refresh_sidebar();
            }
            AppMsg::PerformRename(old_path, new_name) => {
                match utils::rename_path(&old_path, &new_name) {
                    Ok(new_path) => {
                        let old_key = old_path.to_string_lossy().into_owned();
                        let new_key = new_path.to_string_lossy().into_owned();

                        let mut changed = false;

                        // Migration: Transfer persistent folder-specific settings to the new path key
                        if let Some(sort_val) = self.config.ui.folder_sort.remove(&old_key) {
                            self.config.ui.folder_sort.insert(new_key.clone(), sort_val);
                            changed = true;
                        }

                        if let Some(size_val) = self.config.ui.folder_icon_size.remove(&old_key) {
                            self.config.ui.folder_icon_size.insert(new_key, size_val);
                            changed = true;
                        }

                        if changed {
                            utils::save_config(&self.config);
                        }

                        sender.input(AppMsg::Navigate(self.current_path.clone()));
                    }
                    Err(e) => {
                        eprintln!("Failed to rename: {}", e);
                    }
                }
            }
            AppMsg::Activate(_visual_index) => {
                // Reuse the exact same filter-matching logic as Open
                // so Enter key selects the correct file (C instead of A)
                let selection_model = self
                    .files
                    .view
                    .model()
                    .and_then(|m| m.downcast::<gtk::MultiSelection>().ok());

                if let Some(model) = selection_model {
                    let selection = model.selection();
                    if selection.size() > 0 {
                        let visual_index = selection.nth(0);
                        let mut target_path = None;

                        if !self.filter.is_empty() {
                            let query_lc = self.filter.to_lowercase();
                            let mut match_count = 0;

                            for i in 0..self.files.len() {
                                if let Some(wrapper) = self.files.get(i) {
                                    if wrapper.borrow().name.to_lowercase().contains(&query_lc) {
                                        if match_count == visual_index {
                                            target_path = Some(wrapper.borrow().path.clone());
                                            break;
                                        }
                                        match_count += 1;
                                    }
                                }
                            }
                        } else {
                            target_path = self
                                .files
                                .get(visual_index)
                                .map(|w| w.borrow().path.clone());
                        }

                        if let Some(target) = target_path {
                            if target.is_dir() {
                                sender.input(AppMsg::Navigate(target));
                            } else {
                                utils::open_file(target);
                            }
                        }
                    }
                }
            }
            AppMsg::OpenFileProperties(path) => {
                let properties_win = FileProperties::builder().launch(path).detach();
                properties_win.widget().present();
            }
            AppMsg::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.config.ui.show_hidden_by_default = self.show_hidden;
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::CycleSort => {
                self.sort_by = match self.sort_by {
                    SortBy::Name => SortBy::Date,
                    SortBy::Date => SortBy::Size,
                    SortBy::Size => SortBy::Name,
                };

                let path_str = self.current_path.to_string_lossy().to_string();
                self.config
                    .ui
                    .folder_sort
                    .insert(path_str, self.sort_by.clone());
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::CycleFolderPriority => {
                self.config.ui.folders_first = !self.config.ui.folders_first;
                utils::save_config(&self.config);
                let path = self.current_path.clone();
                self.load_path(path, &sender);
            }
            AppMsg::CloseSearchSync => {
                self.search_just_opened = false;
            }
            AppMsg::UpdateFilter(query) => {
                if self.filter == query && !self.search_just_opened {
                    return;
                }

                if self.header_view != "search" && !query.is_empty() {
                    self.header_view = "search".to_string();
                }

                self.filter = query.clone();
                let query_lc = query.to_lowercase();

                self.files.clear_filters();
                if !query_lc.is_empty() {
                    let filter_str = query_lc.clone();
                    self.files
                        .add_filter(move |item| item.name.to_lowercase().contains(&filter_str));
                }

                if !query_lc.is_empty() {
                    if let Some(model) = self
                        .files
                        .view
                        .model()
                        .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                    {
                        if model.n_items() > 0 {
                            model.select_item(0, true);
                        }
                    }
                }
            }
            AppMsg::SearchInput(c) => {
                self.search_just_opened = true;
                self.filter.push(c);
                self.header_view = "search".to_string();
            }
            AppMsg::SearchBackspace => {
                if !self.filter.is_empty() {
                    self.filter.pop();
                    let query = self.filter.clone();
                    sender.input(AppMsg::UpdateFilter(query));
                }
            }
            AppMsg::StartRename(path) => {
                // Locate the item in the model to trigger the 'is_editing' UI state change
                let target_idx = (0..self.files.len())
                    .find(|&i| self.files.get(i).is_some_and(|r| r.borrow().path == path));

                if let Some(idx) = target_idx {
                    if let Some(item_wrapper) = self.files.get(idx) {
                        let mut item = item_wrapper.borrow().clone();
                        item.is_editing = true;
                        self.files.remove(idx);
                        self.files.insert(idx, item);
                    }
                }
            }
            AppMsg::TriggerRenameSelection => {
                if let Some(path) = self.get_selected_path() {
                    sender.input(AppMsg::StartRename(path));
                }
            }
            AppMsg::SwitchHeader(view_name) => {
                self.header_view = view_name;
                if self.header_view != "search" {
                    self.filter = String::new();
                    self.search_just_opened = true;
                    self.files.clear_filters();
                    sender.input(AppMsg::UpdateFilter(String::new()));
                }
            }
            AppMsg::ShowHelp => {
                let help_win = crate::help::HelpWindow::builder().launch(()).detach();
                help_win.widget().present();
            }
            AppMsg::PrepareContextMenu(x, y, path) => {
                let sender_ctx = sender.clone();
                // Performance: MIME detection is a blocking I/O operation; offload to a thread
                relm4::spawn_blocking(move || {
                    let mime = path
                        .as_ref()
                        .map(|p| utils::get_mime_type(p))
                        .unwrap_or_else(|| "inode/directory".to_string());
                    sender_ctx.input(AppMsg::ShowContextMenu { x, y, path, mime });
                });
            }
            AppMsg::ShowContextMenu { x, y, path, mime } => {
                self.active_item_path = path.clone();
                let is_in_trash = self.current_path.to_string_lossy().starts_with("trash://");

                let menu = gio::Menu::new();

                for action in &self.menu_actions {
                    let mut matches = false;

                    // Filtering: Determine which context menu actions are valid for the current file type/location
                    if is_in_trash {
                        if action.mime_types.contains(&"trash".to_string()) {
                            matches = true;
                        }
                    } else {
                        for allowed_mime in &action.mime_types {
                            if allowed_mime == "trash" {
                                continue;
                            }

                            matches = match allowed_mime.as_str() {
                                "*" | "all" => true,
                                "image/all" | "image/*" => mime.starts_with("image/"),
                                "video/all" | "video/*" => mime.starts_with("video/"),
                                "application/all" | "application/*" => {
                                    mime.starts_with("application/")
                                }
                                "text/all" | "text/*" => {
                                    mime.starts_with("text/")
                                        || gio::content_type_is_a(&mime, "text/plain")
                                        || mime == "inode/x-empty"
                                }
                                "folder" | "directory" => mime == "inode/directory",
                                "file" => mime != "inode/directory",
                                t => t == mime,
                            };
                            if matches {
                                break;
                            }
                        }
                    }

                    if matches {
                        let full_action_name = format!("win.{}", action.action_name);
                        menu.append(Some(&action.label), Some(&full_action_name));
                        if let Some(g_action) = self.action_group.lookup_action(&action.action_name)
                        {
                            if let Some(simple) = g_action.downcast_ref::<gio::SimpleAction>() {
                                simple.set_enabled(true);
                            }
                        }
                    }
                }

                self.context_menu_popover.set_menu_model(Some(&menu));
                self.context_menu_popover
                    .set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                self.context_menu_popover.popup();
            }
            AppMsg::ExecuteCommand(cmd_template) => {
                let mut targets = Vec::new();

                // Multi-selection: Extract all selected paths from the GtkSelectionModel
                if let Some(model) = self
                    .files
                    .view
                    .model()
                    .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                {
                    let bitset = model.selection();
                    let n = bitset.size();
                    for i in 0..n {
                        let pos = bitset.nth(i as u32);
                        if let Some(wrapper) = self.files.get(pos) {
                            targets.push(wrapper.borrow().path.clone());
                        }
                    }
                }

                let final_targets = if let Some(active) = &self.active_item_path {
                    if targets.contains(active) {
                        targets
                    } else {
                        vec![active.clone()]
                    }
                } else {
                    vec![self.current_path.clone()]
                };

                // Shell expansion: Replace templates (%p for paths, %d for current directory)
                if final_targets.len() == 1 {
                    utils::run_custom_command(&cmd_template, &final_targets[0]);
                } else if !final_targets.is_empty() {
                    let paths_arg = final_targets
                        .iter()
                        .map(|p| format!("'{}'", p.to_string_lossy().replace("'", "'\\''")))
                        .collect::<Vec<_>>()
                        .join(" ");

                    let mut cmd = cmd_template.replace("%p", &paths_arg);
                    if cmd.contains("%d") {
                        cmd = cmd.replace(
                            "%d",
                            &format!(
                                "'{}'",
                                self.current_path.to_string_lossy().replace("'", "'\\''")
                            ),
                        );
                    }

                    let _ = std::process::Command::new("sh").arg("-c").arg(cmd).spawn();
                }
            }
            AppMsg::Zoom(delta) => {
                let change = if delta > 0.0 { -16 } else { 16 };
                let new_size = (self.current_icon_size + change).clamp(32, 512);
                if new_size != self.current_icon_size {
                    self.current_icon_size = new_size;

                    let path_str = self.current_path.to_string_lossy().to_string();
                    self.config.ui.folder_icon_size.insert(path_str, new_size);
                    utils::save_config(&self.config);

                    // Refresh existing widgets to reflect the new icon scale immediately
                    for i in 0..self.files.len() {
                        if let Some(item_wrapper) = self.files.get(i as u32) {
                            let mut item = item_wrapper.borrow().clone();
                            item.icon_size = new_size;
                            self.files.remove(i as u32);
                            self.files.insert(i as u32, item);
                        }
                    }
                }
            }
            AppMsg::HandleDrop {
                source_path,
                dest_path,
            } => {
                let file_name = source_path.file_name().unwrap();
                let final_dest = dest_path.join(file_name);

                if source_path != final_dest {
                    if let Err(e) = std::fs::rename(&source_path, &final_dest) {
                        eprintln!("[DnD Error] Failed to move {:?}: {}", source_path, e);
                    }
                }
                sender.input(AppMsg::Refresh);
            }
            AppMsg::ToggleSingleClick => {
                self.config.ui.single_click = !self.config.ui.single_click;
                self.files
                    .view
                    .set_single_click_activate(self.config.ui.single_click);
                utils::save_config(&self.config);
            }
            AppMsg::Navigate(path) => {
                let path_str = path.to_string_lossy();

                // Validate path existence (except for virtual trash URI)
                if !path.exists() && !path_str.starts_with("trash://") {
                    return;
                }

                if (path.is_dir() || path_str.starts_with("trash://")) && path != self.current_path
                {
                    let old_path = std::mem::replace(&mut self.current_path, path.clone());

                    // 1. Update the recent navigation stack
                    self.recent_stack.retain(|p| p != &path && p != &old_path);
                    self.recent_stack.push_front(old_path.clone());
                    self.recent_stack.truncate(9);

                    // 2. ABSOLUTE RESET: Clear search state before loading new dir
                    self.filter.clear();
                    self.files.clear_filters();

                    // 3. Reset UI state: Close search view and show breadcrumbs
                    if self.header_view == "search" {
                        self.header_view = "path".to_string();
                    }

                    // 4. Update internal state and history
                    self.history.push(old_path);
                    self.forward_stack.clear();

                    // 5. Trigger the physical load of the new directory
                    self.load_path(path, &sender);
                    self.update_breadcrumbs();

                    // 6. NEW: Auto-select first item and grab focus for keyboard navigation
                    let view = self.files.view.clone();
                    glib::idle_add_local_once(move || {
                        view.grab_focus();
                        if let Some(model) = view
                            .model()
                            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                        {
                            if model.n_items() > 0 {
                                model.select_item(0, true);
                            }
                        }
                    });
                }
            }
            AppMsg::JumpToRecent(rank) => {
                let target_index = if rank == 0 { 0 } else { rank - 1 };

                if let Some(target_path) = self.recent_stack.get(target_index).cloned() {
                    if rank != 0 && target_path == self.current_path {
                        return;
                    }
                    sender.input(AppMsg::Navigate(target_path));
                }
            }
            AppMsg::CycleRecent(delta) => {
                if self.recent_stack.len() < 2 {
                    return;
                }

                let current_pos = self
                    .recent_stack
                    .iter()
                    .position(|p| p == &self.current_path)
                    .unwrap_or(0);

                let len = self.recent_stack.len() as i32;
                let next_idx = (current_pos as i32 + delta).rem_euclid(len) as usize;

                if let Some(target) = self.recent_stack.get(next_idx).cloned() {
                    self.load_path(target, &sender);
                    self.update_breadcrumbs();
                }
            }
            AppMsg::AddExclusive => {
                let path_to_add = self
                    .get_selected_path()
                    .unwrap_or_else(|| self.current_path.clone());

                if !self.exclusive_list.contains(&path_to_add) {
                    self.exclusive_list.push(path_to_add);
                    if self.exclusive_index.is_none() {
                        self.exclusive_index = Some(self.exclusive_list.len() - 1);
                    }
                    self.refresh_sidebar();
                }
            }
            AppMsg::ClearExclusive => {
                self.exclusive_list.clear();
                self.exclusive_index = None;
                self.refresh_sidebar();
            }
            AppMsg::NextExclusive => {
                if !self.exclusive_list.is_empty() {
                    let new_idx = match self.exclusive_index {
                        Some(i) => (i + 1) % self.exclusive_list.len(),
                        None => 0,
                    };
                    self.exclusive_index = Some(new_idx);
                    let target = self.exclusive_list[new_idx].clone();
                    sender.input(AppMsg::Navigate(target));
                }
            }
            AppMsg::PrevExclusive => {
                if !self.exclusive_list.is_empty() {
                    let new_idx = match self.exclusive_index {
                        Some(i) if i > 0 => i - 1,
                        _ => self.exclusive_list.len() - 1,
                    };
                    self.exclusive_index = Some(new_idx);
                    let target = self.exclusive_list[new_idx].clone();
                    sender.input(AppMsg::Navigate(target));
                }
            }
            AppMsg::ThumbnailReady {
                name,
                texture,
                load_id,
            } => {
                // Consistency check: Ignore thumbnails if the user has navigated to a new folder (load_id mismatch)
                if load_id == self.load_id.load(Ordering::SeqCst) {
                    let target_idx = (0..self.files.len())
                        .find(|&i| self.files.get(i).is_some_and(|r| r.borrow().name == name));
                    if let Some(idx) = target_idx {
                        if let Some(item_wrapper) = self.files.get(idx) {
                            let mut item = item_wrapper.borrow().clone();
                            item.thumbnail = Some(texture);
                            self.files.remove(idx);
                            self.files.insert(idx, item);
                        }
                    }
                }
            }
            AppMsg::GoBack => {
                if let Some(prev) = self.history.pop() {
                    self.forward_stack.push(self.current_path.clone());
                    self.load_path(prev, &sender);
                }
            }
            AppMsg::GoForward => {
                if let Some(next) = self.forward_stack.pop() {
                    self.history.push(self.current_path.clone());
                    self.load_path(next, &sender);
                }
            }
            AppMsg::Refresh => {
                let p = self.current_path.clone();
                self.load_path(p, &sender);
            }
            AppMsg::Open(_index) => {
                let selection_model = self
                    .files
                    .view
                    .model()
                    .and_then(|m| m.downcast::<gtk::MultiSelection>().ok());

                if let Some(model) = selection_model {
                    let selection = model.selection();
                    if selection.size() > 0 {
                        // This is the index in the FILTERED list (e.g., 0)
                        let visual_index = selection.nth(0);

                        let mut target_path = None;

                        if !self.filter.is_empty() {
                            let query_lc = self.filter.to_lowercase();
                            let mut match_count = 0;

                            for i in 0..self.files.len() {
                                if let Some(wrapper) = self.files.get(i as u32) {
                                    // Must match the exact logic used in UpdateFilter
                                    if wrapper.borrow().name.to_lowercase().contains(&query_lc) {
                                        if match_count == visual_index {
                                            target_path = Some(wrapper.borrow().path.clone());
                                            break;
                                        }
                                        match_count += 1;
                                    }
                                }
                            }
                        } else {
                            // No filter active: safe to use direct index
                            // (Assuming currently unsorted, or sort matches insertion order)
                            if let Some(wrapper) = self.files.get(visual_index) {
                                target_path = Some(wrapper.borrow().path.clone());
                            }
                        }

                        if let Some(target) = target_path {
                            if target.is_dir() {
                                sender.input(AppMsg::Navigate(target));
                            } else {
                                utils::open_file(target);
                            }
                        }
                    }
                }
            }
            AppMsg::EmptyTrash => {
                let root = gio::File::for_uri("trash:///");
                if let Ok(enumerator) = root.enumerate_children(
                    "standard::name",
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                ) {
                    for info in enumerator.flatten() {
                        let _ = root.child(info.name()).delete(gio::Cancellable::NONE);
                    }
                }
                sender.input(AppMsg::Refresh);
            }
            AppMsg::RestoreItem(_) => {
                sender.input(AppMsg::Refresh);
            }
        }
    }
}
