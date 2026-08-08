use crate::model::{AppMsg, FluxApp};
use gtk::gio;
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

        // Keep the open dialog up-to-date without waiting for the 4 Hz ticker.
        if let Some(dialog) = &mut self.transfer_dialog {
            dialog.refresh();
        }
    }

    pub fn handle_task_completed(&mut self, id: u64) {
        self.task_queue.remove(id);

        // Refresh - this will auto-close the dialog if the queue is now empty.
        if let Some(dialog) = &mut self.transfer_dialog {
            dialog.refresh();
        }
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

    /// Show the transfer dialog unconditionally.
    ///
    /// Idempotent: if a dialog is already open this is a no-op.
    /// Also a no-op if the queue has already drained (race between the 2-second
    /// delay timer and `TaskCompleted`).
    pub fn handle_show_transfer_dialog(&mut self) {
        if self.transfer_dialog.is_some() {
            return;
        }
        // Don't open an empty dialog - the task may have already finished.
        if self.task_queue.is_empty() {
            return;
        }

        let dialog = crate::ui::transfer_dialog::create_transfer_dialog(
            self.task_queue.clone(),
            // `SENDER` is the global channel into the AppMsg update loop.
            crate::model::SENDER
                .get()
                .expect("SENDER not initialised")
                .clone(),
        );
        self.transfer_dialog = Some(dialog);
    }

    /// Show the transfer dialog only if task `id` is still active.
    ///
    /// This is the handler for the time-based fallback: a `tokio::spawn` fires
    /// this message 2 s after every paste operation starts.  If the task
    /// already completed the queue will be empty for that id and we do nothing.
    pub fn handle_show_transfer_dialog_if_active(&mut self, id: u64) {
        if self.transfer_dialog.is_some() {
            return;
        }
        // Check whether this specific task is still alive.
        let still_active = self
            .task_queue
            .snapshot()
            .iter()
            .any(|(task_id, _)| *task_id == id);

        if still_active {
            self.handle_show_transfer_dialog();
        }
    }

    /// Called when the transfer dialog window is closed (via the × button or
    /// because the queue drained).
    ///
    /// Drops the handle, which removes the 4 Hz ticker automatically via `Drop`.
    pub fn handle_transfer_dialog_closed(&mut self) {
        if let Some(mut dialog) = self.transfer_dialog.take() {
            // Stop the ticker before dropping so the glib source is properly
            // removed even if the window was closed by the user rather than
            // programmatically.
            dialog.close();
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

    /// Returns `true` when the transfer dialog should be shown as a button
    /// in the footer: there are active tasks, but the dialog is not open.
    pub fn show_transfer_button(&self) -> bool {
        self.task_queue.summary().is_some() && self.transfer_dialog.is_none()
    }
}
