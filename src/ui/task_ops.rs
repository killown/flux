use crate::model::{AppMsg, FluxApp};
use gtk::gio;
use libc;
use relm4::prelude::*;

impl FluxApp {
    pub fn handle_task_progress(
        &mut self,
        id: u64,
        label: String,
        current: u64,
        total: u64,
        total_items: usize,
        cancellable: gio::Cancellable,
    ) {
        self.task_queue
            .update(id, label, current, total, total_items, cancellable);

        if let Some(dialog) = &mut self.transfer_dialog {
            dialog.refresh();
        }
    }

    pub fn handle_cancel_task(&mut self, id: u64, sender: &AsyncComponentSender<Self>) {
        let pid = self
            .task_queue
            .snapshot()
            .iter()
            .find(|(tid, _)| *tid == id)
            .and_then(|(_, task)| task.pid)
            .filter(|&p| p != 0);

        self.task_queue.cancel(id);

        if let Some(pid) = pid {
            eprintln!("[cancel] pgid={} kill={}", pid, unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL)
            });
        } else {
            eprintln!("[cancel] pid=None");
        }

        sender.input(AppMsg::SelectionChanged);
    }

    pub fn handle_task_completed(&mut self, id: u64) {
        self.task_queue.remove(id);

        if let Some(dialog) = &mut self.transfer_dialog {
            dialog.refresh();
        }
    }

    pub fn handle_cancel_all_tasks(&mut self, sender: &AsyncComponentSender<Self>) {
        for (_, task) in self.task_queue.snapshot() {
            if let Some(pid) = task.pid.filter(|&p| p != 0) {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
        }
        self.task_queue.cancel_all();
        sender.input(AppMsg::SelectionChanged);
    }

    pub fn handle_task_queue_tick(&mut self, sender: &AsyncComponentSender<Self>) {
        match self.task_queue.summary() {
            Some((1, 1, pct)) => {
                self.selection_status = format!("[Copying 1 file | {:.0}%]", pct * 100.0);
            }
            Some((1, items, pct)) => {
                self.selection_status = format!("[Copying {} files | {:.0}%]", items, pct * 100.0);
            }
            Some((n, items, pct)) => {
                self.selection_status =
                    format!("[{} operations, {} files | {:.0}%]", n, items, pct * 100.0);
            }
            None => {
                if self.selection_status.starts_with('[') {
                    self.selection_status = String::new();
                    sender.input(AppMsg::SelectionChanged);
                }
            }
        }
    }

    pub fn handle_show_transfer_dialog(&mut self) {
        if self.transfer_dialog.is_some() {
            return;
        }
        if self.task_queue.is_empty() {
            return;
        }

        let dialog = crate::ui::transfer_dialog::create_transfer_dialog(
            self.task_queue.clone(),
            crate::model::SENDER
                .get()
                .expect("SENDER not initialised")
                .clone(),
        );
        self.transfer_dialog = Some(dialog);
    }

    pub fn handle_show_transfer_dialog_if_active(&mut self, id: u64) {
        if self.transfer_dialog.is_some() {
            return;
        }
        let still_active = self
            .task_queue
            .snapshot()
            .iter()
            .any(|(task_id, _)| *task_id == id);

        if still_active {
            self.handle_show_transfer_dialog();
        }
    }

    pub fn handle_transfer_dialog_closed(&mut self) {
        if let Some(mut dialog) = self.transfer_dialog.take() {
            dialog.close();
        }
    }

    pub fn handle_refresh_path(&mut self, sender: &AsyncComponentSender<Self>) {
        if self.is_content_searching {
            return;
        }

        self.is_loading = true;
        let p = self.current_path.clone();
        let path_str = p.to_string_lossy();

        if crate::services::network::is_network_uri(&p) {
            self.load_network(&path_str, None, sender.clone());
        } else {
            self.load_path(p, sender);
        }
    }

    pub fn show_transfer_button(&self) -> bool {
        self.task_queue.summary().is_some() && self.transfer_dialog.is_none()
    }
}
