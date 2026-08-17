//! Background task queue for tracking concurrent file I/O operations.
//!
//! Provides a thread-safe accumulator that decouples high-frequency progress
//! callbacks from the GTK main loop update rate.

use gtk::gio;
use gtk::gio::prelude::*;
use libc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ─── Speed / ETA ─────────────────────────────────────────────────────────────

/// Maximum age of a speed sample (seconds). Samples older than this are dropped.
const SPEED_WINDOW_SECS: f64 = 2.0;
/// Maximum number of samples retained (ring buffer cap).
const SPEED_RING_CAP: usize = 64;

/// Fixed-capacity ring buffer that computes a smoothed transfer rate.
///
/// Only samples within the last [`SPEED_WINDOW_SECS`] contribute to the
/// average, so the rate adapts quickly without the "999 hours remaining"
/// instability of a purely instantaneous delta.
#[derive(Debug, Clone)]
pub struct SpeedWindow {
    /// Circular buffer of `(timestamp, bytes_transferred_at_that_point)`.
    samples: Vec<(Instant, u64)>,
    head: usize,
    len: usize,
}

impl Default for SpeedWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeedWindow {
    pub fn new() -> Self {
        Self {
            samples: Vec::with_capacity(SPEED_RING_CAP),
            head: 0,
            len: 0,
        }
    }

    /// Record a new cumulative byte count.
    pub fn push(&mut self, bytes: u64) {
        let now = Instant::now();
        if self.samples.len() < SPEED_RING_CAP {
            self.samples.push((now, bytes));
            self.len += 1;
        } else {
            self.samples[self.head] = (now, bytes);
            self.head = (self.head + 1) % SPEED_RING_CAP;
        }
    }

    /// Returns the smoothed transfer rate in **bytes per second**.
    ///
    /// Uses all samples within the last [`SPEED_WINDOW_SECS`] seconds.
    /// Returns `0.0` if fewer than two qualifying samples exist.
    pub fn bytes_per_sec(&self) -> f64 {
        if self.len < 2 {
            return 0.0;
        }

        let now = Instant::now();
        let cutoff = SPEED_WINDOW_SECS;

        // Collect samples within the window (in insertion order).
        let mut window: Vec<(Instant, u64)> = self
            .samples
            .iter()
            .copied()
            .filter(|(t, _)| now.duration_since(*t).as_secs_f64() <= cutoff)
            .collect();

        window.sort_by_key(|(t, _)| *t);

        if window.len() < 2 {
            return 0.0;
        }

        let (t0, b0) = window[0];
        let (t1, b1) = window[window.len() - 1];
        let dt = t1.duration_since(t0).as_secs_f64();

        if dt < 1e-6 || b1 <= b0 {
            return 0.0;
        }

        (b1 - b0) as f64 / dt
    }
}

// ─── Formatting helpers ───────────────────────────────────────────────────────

/// Formats a byte count as a human-readable string (`123 B`, `1.4 MB`, etc.).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;
    match bytes {
        b if b >= GB => format!("{:.1} GB", b as f64 / GB as f64),
        b if b >= MB => format!("{:.1} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{:.1} KB", b as f64 / KB as f64),
        b => format!("{} B", b),
    }
}

/// Formats a duration as `HH:MM:SS` or `MM:SS`.
pub fn format_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

// ─── Task ─────────────────────────────────────────────────────────────────────

/// A single in-flight background operation.
#[derive(Debug, Clone)]
pub struct Task {
    /// The unique action name from `menu_actions` (e.g., `custom_0`), used to toggle `no_command_dialog`.
    pub action_name: Option<String>,
    /// Human-readable description (e.g. filename or "3 files").
    pub label: String,
    /// Full command string (for command tasks).
    pub full_command: Option<String>,
    /// Bytes transferred so far.
    pub current: u64,
    /// Total bytes for this operation.
    pub total: u64,
    /// Number of files within this logical operation.
    pub total_items: usize,
    /// GIO cancellable token, calling `.cancel()` aborts the underlying I/O.
    pub cancellable: gio::Cancellable,
    /// Wall-clock start time for elapsed display.
    #[allow(dead_code)]
    pub started_at: Instant,
    /// Sliding-window speed accumulator.
    pub speed: SpeedWindow,
    /// Process ID if this is a command task (rather than a file operation).
    pub pid: Option<u32>,
    /// Captured output lines (stdout + stderr) for command tasks.
    pub output: Vec<String>,
}

// ─── TaskQueue ────────────────────────────────────────────────────────────────

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
                    .map(|t| (t.cancellable.clone(), t.pid.filter(|&p| p != 0)))
                    .collect();
                map.clear();
                collected
            }
            Err(_) => return,
        };

        for (cancellable, pid) in tasks {
            cancellable.cancel();
            if let Some(pid) = pid {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
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
