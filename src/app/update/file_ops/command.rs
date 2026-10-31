use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, CommandMsg, FileOpsMsg, NavigationMsg, TasksMsg, WindowMsg};
use crate::ui::constants;
use crate::utils::helpers::shell::{expand_template, shell_safe, Placeholder};
use adw::gio::prelude::*;
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{sleep, Duration};

/// Pure helper: builds the final shell command string and human-readable task label.
pub fn build_execution_command(
    cmd_template: &str,
    targets: &[PathBuf],
    current_path: &Path,
    line_number: usize,
) -> (String, String) {
    crate::hit!("build_execution_command");
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

        let cmd = match expand_template(cmd_template, |token: &str| match token {
            constants::TEMPLATE_PATHS => Placeholder::Shell(p_arg.clone()),
            constants::TEMPLATE_CWD => Placeholder::Shell(d_arg.clone()),
            "%f" => Placeholder::Shell(f_arg.clone()),
            "%l" => Placeholder::Plain(line_number.to_string()),
            _ => Placeholder::Unknown,
        }) {
            Some(cmd) => cmd,
            None => return (String::new(), String::new()),
        };

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

        let cmd = match expand_template(cmd_template, |token: &str| match token {
            constants::TEMPLATE_PATHS => Placeholder::Shell(paths_arg.clone()),
            constants::TEMPLATE_CWD => match shell_safe(&current_path.to_string_lossy()) {
                Some(cwd_arg) => Placeholder::Shell(cwd_arg),
                None => Placeholder::Reject,
            },
            "%l" => Placeholder::Plain("0".to_string()),
            _ => Placeholder::Unknown,
        }) {
            Some(cmd) => cmd,
            None => return (String::new(), String::new()),
        };

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
        crate::hit!("resolve_command_targets");
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
        self.handle_execute_command_with(cmd_template, None, sender);
    }

    /// Like [`Self::handle_execute_command`], but lets the caller override the
    /// `no_command_dialog` flag. `None` falls back to the matching `menu_actions` entry.
    pub fn handle_execute_command_with(
        &self,
        cmd_template: String,
        no_command_dialog_override: Option<bool>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_execute_command_with");
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

        if cmd_template == "builtin::install_fonts" {
            let fonts: Vec<PathBuf> = final_targets
                .into_iter()
                .filter(|p| crate::infra::fonts::is_font_path(p))
                .collect();
            if !fonts.is_empty() {
                sender.input(AppMsg::FileOps(FileOpsMsg::InstallFonts(fonts)));
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
            sender.input(AppMsg::Window(WindowMsg::ShowToast(
                "Cannot run command: filename contains unsafe characters".into(),
            )));
            return;
        }

        let no_command_dialog = no_command_dialog_override.unwrap_or_else(|| {
            self.menu_actions
                .iter()
                .find(|action| action.command == cmd_template)
                .map(|a| a.no_command_dialog)
                .unwrap_or(false)
        });

        // Check if this command should be completely untracked (e.g. file properties)
        let bypass_tracking = no_command_dialog || !should_track_in_transfer_dialog(&cmd_template);

        let task_id = if !bypass_tracking {
            let id = crate::infra::tasks::NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
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
                s_delay.input(AppMsg::Command(CommandMsg::ShowCommandDialogIfActive(
                    task_id_delay,
                )));
            });

            Some(id)
        } else {
            None
        };

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
                    sender_cmd.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                        "Failed to spawn command: {}",
                        e
                    ))));
                    if let Some(id) = task_id {
                        sender_cmd.input(AppMsg::Tasks(TasksMsg::TaskCompleted(id)));
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
                        sender_out.input(AppMsg::Command(CommandMsg::CommandOutput {
                            id,
                            line,
                            is_stderr: false,
                        }));
                    }
                }
            });

            let sender_err = sender_cmd.clone();
            let stderr_task = tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    if let Some(id) = task_id {
                        sender_err.input(AppMsg::Command(CommandMsg::CommandOutput {
                            id,
                            line,
                            is_stderr: true,
                        }));
                    }
                }
            });

            let status = child.wait().await;
            let success = status.as_ref().map(|s| s.success()).unwrap_or(false);
            let exit_code = status.ok().and_then(|s| s.code());

            stdout_task.abort();
            stderr_task.abort();

            if let Some(id) = task_id {
                sender_cmd.input(AppMsg::Command(CommandMsg::CommandFinished {
                    id,
                    success,
                    exit_code,
                }));
            }

            if let Some(msg) = toast_msg {
                sender_cmd.input(AppMsg::Window(WindowMsg::ShowToast(msg)));
            } else if !success {
                sender_cmd.input(AppMsg::Window(WindowMsg::ShowToast(
                    "Command failed".to_string(),
                )));
            }

            if needs_refresh {
                sender_cmd.input(AppMsg::Navigation(NavigationMsg::Refresh));
            }
        });
    }
}
