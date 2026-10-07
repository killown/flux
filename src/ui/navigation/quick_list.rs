use crate::model::{AppMsg, FluxApp};
use gtk::gio;
use gtk::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn handle_perform_quick_transfer(
        &mut self,
        dest: PathBuf,
        is_cut: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        let sources = self.resolve_command_targets();
        if sources.is_empty() || !dest.is_dir() {
            return;
        }

        if is_cut {
            self.handle_drop_items(sources, dest, sender);
        } else {
            let sender_clone = sender.clone();
            relm4::spawn_blocking(move || {
                let mut count = 0;
                for src_path in sources {
                    if let Some(name) = src_path.file_name() {
                        let dst_path = dest.join(name);
                        if src_path == dst_path {
                            continue;
                        }
                        let src_file = gtk::gio::File::for_path(&src_path);
                        let dst_file = gtk::gio::File::for_path(&dst_path);

                        if src_file
                            .copy(
                                &dst_file,
                                gtk::gio::FileCopyFlags::OVERWRITE
                                    | gtk::gio::FileCopyFlags::ALL_METADATA,
                                gtk::gio::Cancellable::NONE,
                                None,
                            )
                            .is_ok()
                        {
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    sender_clone.input(AppMsg::ShowToast(format!(
                        "Copied {} item(s) to {}",
                        count,
                        dest.file_name().unwrap_or_default().to_string_lossy()
                    )));
                    sender_clone.input(AppMsg::Refresh);
                }
            });
        }
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

            // ── Right-Click Context Menu on Quick-List Tab ──
            let right_click = gtk::GestureClick::new();
            right_click.set_button(3);

            let s_rc = sender.clone();
            let target_path = path.clone();

            right_click.connect_released(move |gesture, _, x, y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);

                let popover = gtk::PopoverMenu::builder().has_arrow(true).build();
                let menu = gio::Menu::new();
                let action_group = gio::SimpleActionGroup::new();

                menu.append(
                    Some(&format!("󰑮   {}", crate::i18n::tr("Set to Current Folder"))),
                    Some("slot.update"),
                );
                let act_update = gio::SimpleAction::new("update", None);
                let s_update = s_rc.clone();
                act_update.connect_activate(move |_, _| {
                    s_update.input(AppMsg::UpdateExclusiveSlot(idx));
                });
                action_group.add_action(&act_update);

                menu.append(
                    Some(&format!("󱇤   {}", crate::i18n::tr("Open in New Window"))),
                    Some("slot.new_window"),
                );
                let act_win = gio::SimpleAction::new("new_window", None);
                let win_path = target_path.clone();
                act_win.connect_activate(move |_, _| {
                    crate::utils::helpers::open_new_instance(&win_path);
                });
                action_group.add_action(&act_win);

                menu.append(
                    Some(&format!(
                        "󰅖   {}",
                        crate::i18n::tr("Remove from Quick List")
                    )),
                    Some("slot.remove"),
                );
                let act_rm_action = gio::SimpleAction::new("remove", None);
                let s_rm_action = s_rc.clone();
                let rm_path = target_path.clone();
                act_rm_action.connect_activate(move |_, _| {
                    s_rm_action.input(AppMsg::RemoveQuickItem(rm_path.clone()));
                });
                action_group.add_action(&act_rm_action);

                popover.set_menu_model(Some(&menu));
                if let Some(widget) = gesture.widget() {
                    popover.set_parent(&widget);
                    let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                    popover.set_pointing_to(Some(&rect));
                    widget.insert_action_group("slot", Some(&action_group));
                    popover.popup();
                }
            });
            btn.add_controller(right_click);
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
