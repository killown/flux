use super::speed::SpeedWindow;
use super::task::Task;
use gtk::gio;
use gtk::gio::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Shared, thread-safe registry of all active background operations.
#[derive(Debug, Default)]
pub struct TaskQueue {
    inner: Mutex<HashMap<u64, Task>>,
}

impl TaskQueue {
    /// Inserts or updates a task entry.
    ///
    /// On first insert (unrecognised `id`) the `started_at` clock is set to now.
    /// Subsequent calls update `current` and push a speed sample, but **do not**
    /// overwrite the existing `cancellable`. This ensures that the token used
    /// for the actual I/O remains the one that will be cancelled.
    pub fn update(
        &self,
        id: u64,
        label: String,
        current: u64,
        total: u64,
        total_items: usize,
        cancellable: gio::Cancellable,
    ) {
        if let Ok(mut map) = self.inner.lock() {
            let task = map.entry(id).or_insert_with(|| Task {
                label: label.clone(),
                full_command: None,
                action_name: None,
                current: 0,
                total,
                total_items,
                cancellable: cancellable.clone(),
                started_at: Instant::now(),
                speed: SpeedWindow::new(),
                pid: None,
                output: Vec::new(),
            });
            task.label = label;
            task.current = current;
            if total > 0 {
                task.total = total;
            }
            task.total_items = total_items;
            task.speed.push(current);
        }
    }

    pub fn append_output(&self, id: u64, line: String) {
        if let Ok(mut map) = self.inner.lock() {
            if let Some(task) = map.get_mut(&id) {
                task.output.push(line);
            }
        }
    }

    /// Inserts a command task with a given PID and no progress tracking.
    pub fn insert_command(
        &self,
        id: u64,
        label: String,
        pid: u32,
        full_command: Option<String>,
        action_name: Option<String>,
    ) {
        if let Ok(mut map) = self.inner.lock() {
            map.entry(id).or_insert_with(|| Task {
                label: label.clone(),
                full_command,
                action_name,
                current: 0,
                total: 0,
                total_items: 0,
                cancellable: gio::Cancellable::new(),
                started_at: Instant::now(),
                speed: SpeedWindow::new(),
                pid: Some(pid),
                output: Vec::new(),
            });
        }
    }

    /// Removes a completed task entry.
    pub fn remove(&self, id: u64) {
        if let Ok(mut map) = self.inner.lock() {
            map.remove(&id);
        }
    }

    /// Cancels a single in-flight task and removes it from the queue.
    pub fn cancel(&self, id: u64) {
        if let Ok(mut map) = self.inner.lock() {
            if let Some(task) = map.remove(&id) {
                task.cancellable.cancel();
                // PID killing is handled by the caller (update loop) to avoid
                // blocking the queue lock with signal syscalls.
            }
        }
    }

    /// Cancels every in-flight task and clears the queue.
    pub fn cancel_all(&self) {
        let tasks: Vec<(gio::Cancellable, Option<u32>)> = match self.inner.lock() {
            Ok(mut map) => {
                let collected = map
                    .values()
                    .map(|t| (t.cancellable.clone(), t.pid.filter(|&p| p > 1)))
                    .collect();
                map.clear();
                collected
            }
            Err(_) => return,
        };

        for (cancellable, pid) in tasks {
            cancellable.cancel();
            if let Some(pid) = pid {
                let pid_i32 = pid as i32;
                if pid_i32 > 1 {
                    unsafe {
                        if libc::kill(-pid_i32, libc::SIGKILL) != 0 {
                            libc::kill(pid_i32, libc::SIGKILL);
                        }
                    }
                }
            }
        }
    }

    /// Returns `(operation_count, total_items_across_all_ops, aggregate_progress)`.
    ///
    /// Progress is the mean of all individual task fractions. Returns `None`
    /// when the queue is empty.
    pub fn summary(&self) -> Option<(usize, usize, f64)> {
        let map = self.inner.lock().ok()?;
        if map.is_empty() {
            return None;
        }
        let op_count = map.len();
        let total_items: usize = map.values().map(|t| t.total_items).sum();
        let avg = map
            .values()
            .map(|t| {
                if t.total > 0 {
                    t.current as f64 / t.total as f64
                } else {
                    0.0
                }
            })
            .sum::<f64>()
            / op_count as f64;
        Some((op_count, total_items, avg))
    }

    /// Updates the PID of an existing command task after the child has spawned.
    pub fn update_pid(&self, id: u64, pid: u32) {
        if let Ok(mut map) = self.inner.lock() {
            if let Some(task) = map.get_mut(&id) {
                task.pid = Some(pid);
            }
        }
    }

    /// Returns the number of active tasks in the queue.
    pub fn len(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }

    /// Returns a **sorted** snapshot of all active tasks, cheap clone to avoid
    /// holding the lock across GTK widget operations.
    ///
    /// Entries are sorted by task-id (insertion order proxy) so the dialog
    /// renders tasks in a stable order.
    pub fn snapshot(&self) -> Vec<(u64, Task)> {
        let Ok(map) = self.inner.lock() else {
            return Vec::new();
        };
        let mut entries: Vec<(u64, Task)> =
            map.iter().map(|(id, task)| (*id, task.clone())).collect();
        entries.sort_by_key(|(id, _)| *id);
        entries
    }

    /// Returns `true` if there are no active tasks.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().map(|m| m.is_empty()).unwrap_or(false)
    }
}

/// Wraps a `TaskQueue` in an `Arc` for shared ownership across threads.
pub fn new_queue() -> Arc<TaskQueue> {
    Arc::new(TaskQueue::default())
}
