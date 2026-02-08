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
            AppMsg::UpdateFilter(query) => {
                if self.filter == query {
                    return;
                }
                if self.filter.is_empty() && query.len() == 1 {
                    sender.input(AppMsg::SwitchHeader("search".to_string()));
                }

                self.filter = query.clone();
                let query_lc = query.to_lowercase();

                self.files.clear_filters();
                if !query_lc.is_empty() {
                    self.files
                        .add_filter(move |item| item.name.to_lowercase().contains(&query_lc));
                }
            }
            AppMsg::SearchInput(c) => {
                self.filter.push(c);
                sender.input(AppMsg::UpdateFilter(self.filter.clone()));
            }
            AppMsg::SearchBackspace => {
                if !self.filter.is_empty() {
                    self.filter.pop();
                    sender.input(AppMsg::UpdateFilter(self.filter.clone()));
                }
            }
            AppMsg::StartRename(path) => {
                let target_idx = (0..self.files.len()).find(|&i| {
                    self.files
                        .get(i as u32)
                        .map_or(false, |r| r.borrow().path == path)
                });

                if let Some(idx) = target_idx {
                    if let Some(item_wrapper) = self.files.get(idx as u32) {
                        let mut item = item_wrapper.borrow().clone();
                        item.is_editing = true;
                        self.files.remove(idx as u32);
                        self.files.insert(idx as u32, item);
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
                if self.header_view == "path" {
                    self.filter = String::new();
                    sender.input(AppMsg::Refresh);
                }
            }
            AppMsg::ShowHelp => {
                let help_win = crate::help::HelpWindow::builder().launch(()).detach();
                help_win.widget().present();
            }
            AppMsg::PrepareContextMenu(x, y, path) => {
                let sender_ctx = sender.clone();
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
            AppMsg::Navigate(path) => {
                let path_str = path.to_string_lossy();
                if path.is_dir() || path_str.starts_with("trash://") {
                    self.history.push(self.current_path.clone());
                    self.forward_stack.clear();
                    self.load_path(path, &sender);
                    self.update_breadcrumbs();
                }
            }
            AppMsg::ThumbnailReady {
                name,
                texture,
                load_id,
            } => {
                if load_id == self.load_id.load(Ordering::SeqCst) {
                    let target_idx = (0..self.files.len()).find(|&i| {
                        self.files
                            .get(i as u32)
                            .map_or(false, |r| r.borrow().name == name)
                    });
                    if let Some(idx) = target_idx {
                        if let Some(item_wrapper) = self.files.get(idx as u32) {
                            let mut item = item_wrapper.borrow().clone();
                            item.thumbnail = Some(texture);
                            self.files.remove(idx as u32);
                            self.files.insert(idx as u32, item);
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
            AppMsg::Open(index) => {
                if let Some(item_wrapper) = self.files.get(index) {
                    let item = item_wrapper.borrow();
                    let target = if self.current_path.to_string_lossy().starts_with("trash://") {
                        item.path.clone()
                    } else {
                        self.current_path.join(&item.name)
                    };
                    if target.is_dir() {
                        sender.input(AppMsg::Navigate(target));
                    } else {
                        utils::open_file(target);
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
