use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
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

/// Pure helper: builds the final shell command string and human-readable task label.
pub fn build_execution_command(
    cmd_template: &str,
    targets: &[PathBuf],
    current_path: &Path,
    line_number: usize,
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
            .replace("\"%p\"", &p_arg)
            .replace("'%p'", &p_arg)
            .replace("%p", &p_arg)
            .replace("\"%d\"", &d_arg)
            .replace("'%d'", &d_arg)
            .replace("%d", &d_arg)
            .replace("\"%f\"", &f_arg)
            .replace("'%f'", &f_arg)
            .replace("%f", &f_arg)
            .replace("%l", &line_number.to_string());

        if cmd.contains(constants::TEMPLATE_CWD) {
            let cwd_arg = match shell_safe(&current_path.to_string_lossy()) {
                Some(a) => a,
                None => return (String::new(), String::new()),
            };
            cmd = cmd
                .replace(&format!("\"{}\"", constants::TEMPLATE_CWD), &cwd_arg)
                .replace(&format!("'{}'", constants::TEMPLATE_CWD), &cwd_arg)
                .replace(constants::TEMPLATE_CWD, &cwd_arg);
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

        let mut cmd = cmd_template
            .replace(&format!("\"{}\"", constants::TEMPLATE_PATHS), &paths_arg)
            .replace(&format!("'{}'", constants::TEMPLATE_PATHS), &paths_arg)
            .replace(constants::TEMPLATE_PATHS, &paths_arg);

        if cmd.contains(constants::TEMPLATE_CWD) {
            let cwd_arg = match shell_safe(&current_path.to_string_lossy()) {
                Some(a) => a,
                None => return (String::new(), String::new()),
            };
            cmd = cmd
                .replace(&format!("\"{}\"", constants::TEMPLATE_CWD), &cwd_arg)
                .replace(&format!("'{}'", constants::TEMPLATE_CWD), &cwd_arg)
                .replace(constants::TEMPLATE_CWD, &cwd_arg);
        }

        cmd = cmd.replace("%l", "0");

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
    /// Extracts target paths from selection or active item context.
    pub fn resolve_command_targets(&self) -> Vec<PathBuf> {
        let mut targets = Vec::new();
        let active_files = match self.tabs.get(self.active_tab_index) {
            Some(tab) => &tab.files,
            None => return targets,
        };

        if let Some(model) = active_files
            .view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
        {
            let bitset = model.selection();
            for i in 0..bitset.size() {
                let pos = bitset.nth(i as u32);
                if let Some(wrapper) = active_files.get(pos) {
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

        let is_restore = cmd_template.contains("trash --restore");
        let is_in_trash = self
            .current_path
            .to_string_lossy()
            .starts_with(constants::TRASH_URI);
        let needs_refresh = is_in_trash || is_restore;

        let (final_cmd, label) = build_execution_command(
            &cmd_template,
            &final_targets,
            &current_path,
            self.active_item_line,
        );

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
            let id = crate::ui::ops::paste::NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
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
}
