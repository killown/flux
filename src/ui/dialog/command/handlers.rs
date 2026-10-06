use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

impl FluxApp {
    pub fn handle_toggle_no_command_dialog(&mut self, action_name: String) {
        if let Some(action) = self
            .menu_actions
            .iter_mut()
            .find(|a| a.action_name == action_name)
        {
            action.no_command_dialog = !action.no_command_dialog;
            crate::utils::save_config(&self.config);
            if let Err(e) = crate::utils::save_menu_config(&self.menu_actions) {
                eprintln!("Failed to save menu.rs: {}", e);
            }
        }
    }

    pub fn handle_refresh_command_dialog(&self, action_name: String) {
        if let Some(dialog) = &self.command_dialog {
            if let Some(action) = self
                .menu_actions
                .iter()
                .find(|a| a.action_name == action_name)
            {
                dialog.update_switch_state(action.no_command_dialog);
            }
        }
    }

    pub fn handle_command_output(&self, id: u64, line: String, is_stderr: bool) {
        self.task_queue.append_output(id, line.clone());
        let prefix = if is_stderr { "stderr" } else { "stdout" };
        eprintln!("[task {}] {}: {}", id, prefix, line);

        if let Some(dialog) = &self.command_dialog {
            if dialog.task_id == id {
                dialog.append_output(&line);
            }
        }
    }

    pub fn handle_command_finished(
        &mut self,
        id: u64,
        success: bool,
        exit_code: Option<i32>,
        sender: &AsyncComponentSender<Self>,
    ) {
        if success {
            self.folder_cache
                .remove(&self.cache_key(&self.current_path));
        }
        if !success {
            let msg = if let Some(code) = exit_code {
                format!("Command failed with exit code {}", code)
            } else {
                "Command was terminated".to_string()
            };
            sender.input(AppMsg::ShowToast(msg));
        }

        if let Some(dialog) = &self.command_dialog {
            if dialog.task_id == id {
                self.handle_command_dialog_closed();
            }
        }

        sender.input(AppMsg::TaskCompleted(id));
    }
}
