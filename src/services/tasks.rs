//! Background task queue for tracking concurrent file I/O operations.
//!
//! Provides a thread-safe accumulator that decouples high-frequency progress
//! callbacks from the GTK main loop update rate.

use gtk::gio;
use gtk::gio::prelude::*;
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
    /// Human-readable description (e.g. filename or "3 files").
    pub label: String,
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
                current: 0,
                total,
                total_items,
                cancellable: cancellable.clone(),
                started_at: Instant::now(),
                speed: SpeedWindow::new(),
            });
            task.label = label;
            task.current = current;
            task.total = total;
            task.total_items = total_items;
            // Do NOT update cancellable if the task already existed
            task.speed.push(current);
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
            }
        }
    }

    /// Cancels every in-flight task and clears the queue.
    pub fn cancel_all(&self) {
        if let Ok(mut map) = self.inner.lock() {
            for task in map.values() {
                task.cancellable.cancel();
            }
            map.clear();
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

    /// Returns a **sorted** snapshot of all active tasks - cheap clone to avoid
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
