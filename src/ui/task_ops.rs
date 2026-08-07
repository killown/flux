use crate::model::{AppMsg, FluxApp};
use gtk::gio;
use relm4::prelude::*;

impl FluxApp {
    pub fn handle_task_progress(
        &mut self,
        id: u64,
        current: u64,
        total: u64,
        total_items: usize,
        cancellable: gio::Cancellable,
    ) {
        self.task_queue
            .update(id, current, total, total_items, cancellable);
    }

    pub fn handle_task_completed(&mut self, id: u64) {
        self.task_queue.remove(id);
    }

    pub fn handle_cancel_task(&mut self, id: u64, sender: &AsyncComponentSender<Self>) {
        self.task_queue.cancel(id);
        sender.input(AppMsg::SelectionChanged);
    }

    pub fn handle_cancel_all_tasks(&mut self, sender: &AsyncComponentSender<Self>) {
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

    pub fn handle_refresh_path(&mut self, sender: &AsyncComponentSender<Self>) {
        self.is_loading = true;
        let p = self.current_path.clone();
        let path_str = p.to_string_lossy();

        if crate::services::network::is_network_uri(&p) {
            self.load_network(&path_str, None, sender.clone());
        } else {
            self.load_path(p, sender);
        }
    }
}
