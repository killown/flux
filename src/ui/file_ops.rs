use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use crate::utils;
use adw::gdk;
use adw::gio::prelude::*;
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Handles clipboard Copy and Cut actions by populating standard GTK Clipboard providers.
    pub fn handle_copy_or_cut(&self, is_cut: bool) {
        self.handle_clipboard_action(is_cut);
    }

    /// Reads content from the clipboard and dispatches a paste message.
    pub fn handle_paste_from_clipboard(&self, sender: &AsyncComponentSender<Self>) {
        let Some(display) = gdk::Display::default() else {
            sender.input(AppMsg::ShowToast(
                "No display available for clipboard operation".to_string(),
            ));
            return;
        };

        let clipboard = display.clipboard();
        let s = sender.clone();

        clipboard.read_text_async(None::<&gio::Cancellable>, move |res| {
            if let Ok(Some(text)) = res {
                let mut lines = text.lines();
                let first_line = lines.next().unwrap_or("");

                let is_cut = first_line == "cut";

                let files: Vec<gio::File> = lines
                    .filter(|uri| !uri.is_empty())
                    .map(|uri| gio::File::for_uri(uri.trim_end_matches('\r')))
                    .collect();

                if !files.is_empty() {
                    s.input(AppMsg::PerformPaste { files, is_cut });
                }
            }
        });
    }

    /// Presents a warning dialog when pasting into a location where target folders already exist.
    pub fn show_confirm_replace_paste(
        &self,
        files: Vec<gio::File>,
        conflicts: Vec<String>,
        is_cut: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        let window = gtk::Application::default().active_window();
        let body = if conflicts.len() == 1 {
            format!(
                "\"{}\" already exists in this location. Replace it and merge its contents?",
                conflicts[0]
            )
        } else {
            format!(
                "{} folders already exist in this location. Replace them and merge their contents?",
                conflicts.len()
            )
        };
        let dialog = gtk::MessageDialog::new(
            window.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Warning,
            gtk::ButtonsType::None,
            "Replace Existing Folder?",
        );
        dialog.set_secondary_text(Some(&body));
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);

        let replace_btn = dialog.add_button("Replace", gtk::ResponseType::Accept);
        replace_btn.style_context().add_class("destructive-action");

        let s = sender.clone();
        dialog.connect_response(move |dlg, response| {
            dlg.close();
            if response == gtk::ResponseType::Accept {
                s.input(AppMsg::PerformPasteForced {
                    files: files.clone(),
                    is_cut,
                });
            }
        });
        dialog.present();
    }

    /// Handles file and directory renames with error handling for permissions.
    pub fn handle_perform_rename(
        &mut self,
        old_path: PathBuf,
        new_name: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        match utils::rename_path(&old_path, &new_name) {
            Ok(new_path) => {
                let _ = self.state_db.rename_path(&old_path, &new_path);
                sender.input(AppMsg::Navigate(self.current_path.clone()));
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Permission denied") || msg.contains("Operation not permitted") {
                    sender.input(AppMsg::ShowToast(
                        "Permission denied: Cannot move item to trash.".into(),
                    ));
                } else {
                    sender.input(AppMsg::ShowToast(format!("Trash error: {}", e)));
                }
            }
        }
    }

    /// Executes custom shell commands or built-in actions across single or multiple targets.
    pub fn handle_execute_command(
        &self,
        cmd_template: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        let mut targets = Vec::new();
        if let Some(model) = self
            .files
            .view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
        {
            let bitset = model.selection();
            for i in 0..bitset.size() {
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

        if cmd_template == "builtin::open_with" {
            if let Some(path) = final_targets.first() {
                let file = gio::File::for_path(path);
                if let Ok(info) = file.query_info(
                    "standard::content-type",
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                ) {
                    if let Some(mime) = info.content_type() {
                        let apps = gio::AppInfo::all_for_type(&mime);
                        if let Some(app) = apps.first() {
                            let files: Vec<gio::File> =
                                final_targets.iter().map(gio::File::for_path).collect();
                            let _ = app.launch(&files, None::<&gio::AppLaunchContext>);
                        }
                    }
                }
            }
            return;
        }

        let current_path = self.current_path.clone();
        let toast_msg = self
            .menu_actions
            .iter()
            .find(|action| action.command == cmd_template)
            .and_then(|a| a.toast.clone());

        let needs_refresh = self
            .current_path
            .to_string_lossy()
            .starts_with(constants::TRASH_URI);
        let sender_clone = sender.clone();

        relm4::spawn_blocking(move || {
            if final_targets.len() == 1 {
                Self::run_custom_command_wait(&cmd_template, &final_targets[0]);
            } else if !final_targets.is_empty() {
                let paths_arg = final_targets
                    .iter()
                    .map(|p| format!("'{}'", p.to_string_lossy().replace("'", "'\\''")))
                    .collect::<Vec<_>>()
                    .join(" ");

                let mut cmd = cmd_template.replace(constants::TEMPLATE_PATHS, &paths_arg);
                if cmd.contains(constants::TEMPLATE_CWD) {
                    cmd = cmd.replace(
                        constants::TEMPLATE_CWD,
                        &format!("'{}'", current_path.to_string_lossy().replace("'", "'\\''")),
                    );
                }

                let _ = std::process::Command::new(constants::SHELL_BIN)
                    .arg("-c")
                    .arg(cmd)
                    .status();
            }

            if let Some(msg) = toast_msg {
                sender_clone.input(AppMsg::ShowToast(msg));
            }

            if needs_refresh {
                sender_clone.input(AppMsg::Refresh);
            }
        });
    }

    /// Handles drag-and-drop moves for internal items.
    pub fn handle_drop_items(
        &self,
        source_paths: Vec<PathBuf>,
        dest_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let sender_clone = sender.clone();

        relm4::spawn_blocking(move || {
            for source_path in source_paths {
                if !dest_path.is_dir() {
                    break;
                }

                let Some(file_name) = source_path.file_name() else {
                    continue;
                };

                let final_dest = dest_path.join(file_name);

                if source_path == final_dest {
                    continue;
                }

                let src_file = gio::File::for_path(&source_path);
                let dst_file = gio::File::for_path(&final_dest);

                if let Err(e) = src_file.move_(
                    &dst_file,
                    gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                    gio::Cancellable::NONE,
                    None,
                ) {
                    eprintln!("[DnD Error] Failed to move {:?}: {}", source_path, e);
                }
            }

            sender_clone.input(AppMsg::Refresh);
        });
    }

    /// Handles drag-and-drop moves from external windows/processes.
    pub fn handle_external_drop_items(
        &self,
        source_paths: Vec<PathBuf>,
        dest_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let sender_clone = sender.clone();
        relm4::spawn_blocking(move || {
            for source in source_paths {
                let Some(file_name) = source.file_name() else {
                    continue;
                };

                let final_dest = dest_path.join(file_name);

                if source == final_dest {
                    continue;
                }

                let src_file = gio::File::for_path(&source);
                let dst_file = gio::File::for_path(&final_dest);

                if let Err(e) = src_file.move_(
                    &dst_file,
                    gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                    gio::Cancellable::NONE,
                    None,
                ) {
                    eprintln!("[File Error] External move failed: {}", e);
                }
            }

            sender_clone.input(AppMsg::Refresh);
        });
    }

    /// Deletes all files currently residing in the virtual system trash directory.
    pub fn handle_empty_trash(&self, sender: &AsyncComponentSender<Self>) {
        let root = gio::File::for_uri(constants::TRASH_URI);
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
}
