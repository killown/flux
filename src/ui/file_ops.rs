use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use crate::utils;
use adw::gio::prelude::*;
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{sleep, Duration};

fn shell_safe(s: &str) -> Option<String> {
    if s.contains('\n') || s.contains('\r') || s.contains('\0') {
        return None;
    }
    Some(format!("'{}'", s.replace('\'', "'\\''")))
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Pure helper: builds the final shell command string and human-readable task label.
pub fn build_execution_command(
    cmd_template: &str,
    targets: &[PathBuf],
    current_path: &Path,
) -> (String, String) {
    if targets.len() == 1 {
        let path = &targets[0];
        let path_str = path.to_string_lossy();
        let parent = path.parent().unwrap_or(path).to_string_lossy();
        let filename = path.file_name().unwrap_or_default().to_string_lossy();

        let p_arg = match shell_safe(&path_str) {
            Some(a) => a,
            None => return (String::new(), String::new()),
        };
        let d_arg = match shell_safe(&parent) {
            Some(a) => a,
            None => return (String::new(), String::new()),
        };
        let f_arg = match shell_safe(&filename) {
            Some(a) => a,
            None => return (String::new(), String::new()),
        };

        let mut cmd = cmd_template
            .replace("%p", &p_arg)
            .replace("%d", &d_arg)
            .replace("%f", &f_arg);

        if cmd.contains(constants::TEMPLATE_CWD) {
            cmd = cmd.replace(
                constants::TEMPLATE_CWD,
                &match shell_safe(&current_path.to_string_lossy()) {
                    Some(a) => a,
                    None => return (String::new(), String::new()),
                },
            );
        }

        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Command".to_string());
        (cmd, label)
    } else {
        let paths: Option<Vec<String>> = targets
            .iter()
            .map(|p| shell_safe(&p.to_string_lossy()))
            .collect();
        let paths_arg = match paths {
            Some(v) => v.join(" "),
            None => return (String::new(), String::new()),
        };

        let mut cmd = cmd_template.replace(constants::TEMPLATE_PATHS, &paths_arg);
        if cmd.contains(constants::TEMPLATE_CWD) {
            cmd = cmd.replace(
                constants::TEMPLATE_CWD,
                &match shell_safe(&current_path.to_string_lossy()) {
                    Some(a) => a,
                    None => return (String::new(), String::new()),
                },
            );
        }
        let label = format!("{} items", targets.len());
        (cmd, label)
    }
}

/// Legacy predicate: returns false for commands known to open their own window,
/// keeping backwards compatibility for installs that haven't added `no_command_dialog`
/// to their menu.rs yet.
pub fn should_track_in_transfer_dialog(cmd_template: &str) -> bool {
    !cmd_template.contains("--file-properties")
}

impl FluxApp {
    /// Handles clipboard Copy and Cut actions by populating standard GTK Clipboard providers.
    pub fn handle_copy_or_cut(&self, is_cut: bool, sender: &AsyncComponentSender<Self>) {
        self.handle_clipboard_action(is_cut);
        let cmd = if is_cut {
            "builtin::cut"
        } else {
            "builtin::copy"
        };
        if let Some(toast) = self
            .menu_actions
            .iter()
            .find(|a| a.command == cmd)
            .and_then(|a| a.toast.clone())
        {
            sender.input(AppMsg::ShowToast(toast));
        }
    }

    pub fn handle_extract_archive(&self, sender: &AsyncComponentSender<Self>) {
        let uri = self.current_path.to_string_lossy().to_string();
        let Some((archive_path, _)) = crate::services::archive::parse_archive_uri(&uri) else {
            return;
        };
        let stem = archive_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let parent = archive_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let base_name = format!("{}_extracted", stem);
        let mut dest = parent.join(&base_name);
        let mut counter = 2;
        while dest.exists() {
            dest = parent.join(format!("{}_{}", base_name, counter));
            counter += 1;
        }

        let task_id = crate::ui::paste_ops::NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
        let cancellable = gtk::gio::Cancellable::new();
        let label = format!(
            "Extracting {}",
            archive_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );

        sender.input(AppMsg::TaskProgress {
            id: task_id,
            label,
            current: 0,
            total: 0,
            total_items: 1,
            cancellable,
        });

        let password = self.cached_archive_password.clone();
        let s = sender.clone();
        relm4::spawn_blocking(move || {
            match crate::services::archive::extract_archive(&archive_path, password.as_deref()) {
                Ok(tmp_path) => {
                    let result = std::fs::rename(&tmp_path, &dest).or_else(|e| {
                        if e.raw_os_error() == Some(libc::EXDEV) {
                            copy_dir_all(&tmp_path, &dest)
                                .and_then(|_| std::fs::remove_dir_all(&tmp_path))
                        } else {
                            Err(e)
                        }
                    });
                    s.input(AppMsg::TaskCompleted(task_id));
                    match result {
                        Ok(_) => {
                            s.input(AppMsg::ShowToast(format!(
                                "Extracted to {}",
                                dest.display()
                            )));
                            s.input(AppMsg::InvalidateCacheAndNavigate(dest));
                        }
                        Err(e) => {
                            s.input(AppMsg::ShowToast(format!("Extract failed: {e}")));
                        }
                    }
                }
                Err(e) => {
                    s.input(AppMsg::TaskCompleted(task_id));
                    s.input(AppMsg::ShowToast(format!("Extract failed: {e}")));
                }
            }
        });
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

                let old_key = old_path.to_string_lossy().to_string();
                let new_key = new_path.to_string_lossy().to_string();

                // Re-key custom image overrides so the association survives renames.
                if let Some(v) = self.config.ui.file_icons.remove(&old_key) {
                    self.config.ui.file_icons.insert(new_key.clone(), v);
                }
                // Re-key GTK icon name overrides for directories.
                if let Some(v) = self.config.ui.folder_icons.remove(&old_key) {
                    self.config.ui.folder_icons.insert(new_key, v);
                }

                utils::save_config(&self.config);
                let canon_old = old_path.canonicalize().unwrap_or_else(|_| old_path.clone());
                let canon_new = new_path.canonicalize().unwrap_or_else(|_| new_path.clone());
                self.file_op_history
                    .push_undo(crate::ui::undo_redo::FileOp::Rename {
                        old_path: canon_old,
                        new_path: canon_new,
                        old_name: old_path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        new_name: new_name.clone(),
                    });

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

    /// Extracts target paths from selection or active item context.
    pub fn resolve_command_targets(&self) -> Vec<PathBuf> {
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

        if let Some(active) = &self.active_item_path {
            if targets.contains(active) {
                targets
            } else {
                vec![active.clone()]
            }
        } else if !targets.is_empty() {
            targets
        } else {
            vec![self.current_path.clone()]
        }
    }

    /// Executes a shell command, spawning it as a tracked background task.
    pub fn handle_execute_command(
        &self,
        cmd_template: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        let final_targets = self.resolve_command_targets();

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

        if final_targets.is_empty() {
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

        let (final_cmd, label) =
            build_execution_command(&cmd_template, &final_targets, &current_path);

        if final_cmd.is_empty() {
            sender.input(AppMsg::ShowToast(
                "Cannot run command: filename contains unsafe characters".into(),
            ));
            return;
        }

        // Resolve the no_command_dialog flag from the matching menu action.
        let no_command_dialog = self
            .menu_actions
            .iter()
            .find(|action| action.command == cmd_template)
            .map(|a| a.no_command_dialog)
            .unwrap_or(false);

        // Check if this command should be completely untracked (e.g. file properties)
        let bypass_tracking = no_command_dialog || !should_track_in_transfer_dialog(&cmd_template);

        // Generate task ID only if tracking is needed – defined OUTSIDE the condition
        let task_id = if !bypass_tracking {
            let id = crate::ui::paste_ops::NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
            // Insert into queue – note the full_command parameter
            let action_name = self
                .menu_actions
                .iter()
                .find(|a| a.command == cmd_template)
                .map(|a| a.action_name.clone());
            self.task_queue
                .insert_command(id, label, 0, Some(final_cmd.clone()), action_name);

            // Show command dialog after 2 seconds if still running
            let s_delay = sender.clone();
            let task_id_delay = id;
            relm4::spawn(async move {
                sleep(Duration::from_secs(2)).await;
                s_delay.input(AppMsg::ShowCommandDialogIfActive(task_id_delay));
            });

            Some(id)
        } else {
            None
        };

        // Spawn command asynchronously
        let sender_cmd = sender.clone();
        let task_queue = self.task_queue.clone();

        relm4::spawn(async move {
            let child = unsafe {
                Command::new("sh")
                    .arg("-c")
                    .arg(&final_cmd)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .process_group(0)
                    .pre_exec(|| {
                        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0);
                        Ok(())
                    })
                    .spawn()
            };

            let mut child = match child {
                Ok(c) => c,
                Err(e) => {
                    sender_cmd.input(AppMsg::ShowToast(format!("Failed to spawn command: {}", e)));
                    if let Some(id) = task_id {
                        sender_cmd.input(AppMsg::TaskCompleted(id));
                    }
                    return;
                }
            };

            if let Some(id) = task_id {
                let pid = child.id().unwrap_or(0);
                task_queue.update_pid(id, pid);
            }

            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            let sender_out = sender_cmd.clone();
            let stdout_task = tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    if let Some(id) = task_id {
                        sender_out.input(AppMsg::CommandOutput {
                            id,
                            line,
                            is_stderr: false,
                        });
                    }
                }
            });

            let sender_err = sender_cmd.clone();
            let stderr_task = tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    if let Some(id) = task_id {
                        sender_err.input(AppMsg::CommandOutput {
                            id,
                            line,
                            is_stderr: true,
                        });
                    }
                }
            });

            let status = child.wait().await;
            let success = status.as_ref().map(|s| s.success()).unwrap_or(false);
            let exit_code = status.ok().and_then(|s| s.code());

            stdout_task.abort();
            stderr_task.abort();

            if let Some(id) = task_id {
                sender_cmd.input(AppMsg::CommandFinished {
                    id,
                    success,
                    exit_code,
                });
            }

            if let Some(msg) = toast_msg {
                sender_cmd.input(AppMsg::ShowToast(msg));
            } else if !success {
                sender_cmd.input(AppMsg::ShowToast("Command failed".to_string()));
            }

            if needs_refresh {
                sender_cmd.input(AppMsg::Refresh);
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
        if dest_path.as_os_str().is_empty() || !dest_path.is_dir() {
            return;
        }

        let Ok(dest_canon) = dest_path.canonicalize() else {
            return;
        };

        let sender_clone = sender.clone();

        relm4::spawn_blocking(move || {
            let mut completed_moves = Vec::new();

            for source_path in source_paths {
                let Ok(source_canon) = source_path.canonicalize() else {
                    continue;
                };

                // HARD GUARD: If the file's parent directory is the exact same as the destination, skip!
                if let Some(parent) = source_canon.parent() {
                    if parent == dest_canon {
                        continue;
                    }
                }

                if source_canon == dest_canon {
                    continue;
                }

                let Some(file_name) = source_canon.file_name() else {
                    continue;
                };

                let final_dest = dest_canon.join(file_name);
                if source_canon == final_dest {
                    continue;
                }

                let src_file = gio::File::for_path(&source_canon);
                let dst_file = gio::File::for_path(&final_dest);

                if src_file
                    .move_(
                        &dst_file,
                        gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::ALL_METADATA,
                        gio::Cancellable::NONE,
                        None,
                    )
                    .is_ok()
                {
                    completed_moves.push((source_canon.clone(), final_dest.clone()));
                    sender_clone.input(AppMsg::ItemMoved {
                        old_path: source_canon,
                        new_path: final_dest,
                    });
                } else {
                    eprintln!("[DnD Error] Failed to move {:?}", source_canon);
                }
            }

            if !completed_moves.is_empty() {
                sender_clone.input(AppMsg::MoveSucceeded {
                    items: completed_moves,
                    dest_dir: dest_canon,
                });
            }

            sender_clone.input(AppMsg::Refresh);
        });
    }

    /// Handles drag-and-drop moves from external windows/processes.
    pub fn handle_external_drop_items(
        &self,
        source_paths: Vec<PathBuf>,
        _dest_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        // FORCE the destination to always be the directory currently viewed in Flux,
        // preventing window-level drop targets from accidentally dumping files into /home/neo.
        let dest_path = self.current_path.clone();

        if dest_path.as_os_str().is_empty() || !dest_path.is_dir() {
            return;
        }

        let Ok(dest_canon) = dest_path.canonicalize() else {
            return;
        };

        let sender_clone = sender.clone();
        relm4::spawn_blocking(move || {
            let mut completed_moves = Vec::new();

            for source in source_paths {
                let Ok(source_canon) = source.canonicalize() else {
                    continue;
                };

                // HARD GUARD: If the file is already in the current folder, do not move it!
                if let Some(parent) = source_canon.parent() {
                    if parent == dest_canon {
                        continue;
                    }
                }

                if source_canon == dest_canon {
                    continue;
                }

                let Some(file_name) = source_canon.file_name() else {
                    continue;
                };

                let final_dest = dest_canon.join(file_name);

                if source_canon == final_dest {
                    continue;
                }

                let src_file = gio::File::for_path(&source_canon);
                let dst_file = gio::File::for_path(&final_dest);

                if src_file
                    .move_(
                        &dst_file,
                        gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::ALL_METADATA,
                        gio::Cancellable::NONE,
                        None,
                    )
                    .is_ok()
                {
                    completed_moves.push((source_canon.clone(), final_dest.clone()));
                    sender_clone.input(AppMsg::ItemMoved {
                        old_path: source_canon,
                        new_path: final_dest,
                    });
                } else {
                    eprintln!("[File Error] External move failed");
                }
            }

            if !completed_moves.is_empty() {
                sender_clone.input(AppMsg::MoveSucceeded {
                    items: completed_moves,
                    dest_dir: dest_canon,
                });
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
