//! Background task queue for tracking concurrent file I/O operations.
//!
//! Provides a thread-safe accumulator that decouples high-frequency progress
//! callbacks from the GTK main loop update rate.

use gtk::gio;
use gtk::gio::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A single in-flight background operation.
#[derive(Debug, Clone)]
pub struct Task {
    pub current: u64,
    pub total: u64,
    pub total_items: usize,
    /// GIO cancellable token; calling `.cancel()` aborts the underlying I/O.
    pub cancellable: gio::Cancellable,
}

/// Shared, thread-safe registry of all active background operations.
#[derive(Debug, Default)]
pub struct TaskQueue {
    inner: Mutex<HashMap<u64, Task>>,
}

impl TaskQueue {
    /// Inserts or updates a task entry.
    ///
    /// Args:
    ///     id: Unique monotonic identifier for the operation.
    ///     current: Bytes (or units) transferred so far.
    ///     total: Total bytes (or units) for the operation.
    ///     total_items: Number of files within this logical operation.
    ///     cancellable: GIO cancellable handle associated with this task.
    pub fn update(
        &self,
        id: u64,
        current: u64,
        total: u64,
        total_items: usize,
        cancellable: gio::Cancellable,
    ) {
        if let Ok(mut map) = self.inner.lock() {
            map.insert(
                id,
                Task {
                    current,
                    total,
                    total_items,
                    cancellable,
                },
            );
        }
    }

    /// Removes a completed task entry.
    pub fn remove(&self, id: u64) {
        if let Ok(mut map) = self.inner.lock() {
            map.remove(&id);
        }
    }

    /// Cancels a single in-flight task and removes it from the queue.
    ///
    /// Args:
    ///     id: The task identifier to cancel.
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
}

/// Wraps a `TaskQueue` in an `Arc` for shared ownership across threads.
pub fn new_queue() -> Arc<TaskQueue> {
    Arc::new(TaskQueue::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_queue() -> TaskQueue {
        TaskQueue::default()
    }

    #[test]
    fn test_summary_empty_queue_returns_none() {
        let q = make_queue();
        assert!(q.summary().is_none());
    }

    #[test]
    fn test_update_and_summary_single_task() {
        let q = make_queue();
        let c = gtk::gio::Cancellable::new();
        q.update(1, 50, 100, 3, c);

        let (ops, items, avg) = q.summary().expect("queue must be non-empty");
        assert_eq!(ops, 1);
        assert_eq!(items, 3);
        assert!((avg - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_remove_empties_queue() {
        let q = make_queue();
        let c = gtk::gio::Cancellable::new();
        q.update(1, 10, 10, 1, c);
        q.remove(1);
        assert!(q.summary().is_none());
    }

    #[test]
    fn test_cancel_all_clears_queue() {
        let q = make_queue();
        q.update(1, 0, 100, 1, gtk::gio::Cancellable::new());
        q.update(2, 0, 200, 2, gtk::gio::Cancellable::new());
        q.cancel_all();
        assert!(q.summary().is_none());
    }

    #[test]
    fn test_summary_averages_multiple_tasks() {
        let q = make_queue();
        // Task A: 0% complete
        q.update(1, 0, 100, 1, gtk::gio::Cancellable::new());
        // Task B: 100% complete
        q.update(2, 200, 200, 1, gtk::gio::Cancellable::new());

        let (ops, items, avg) = q.summary().unwrap();
        assert_eq!(ops, 2);
        assert_eq!(items, 2);
        // Mean of 0.0 and 1.0
        assert!((avg - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_summary_zero_total_contributes_zero_progress() {
        let q = make_queue();
        // total == 0 must not panic and must count as 0.0 progress
        q.update(1, 0, 0, 1, gtk::gio::Cancellable::new());
        let (_, _, avg) = q.summary().unwrap();
        assert!((avg - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_remove_nonexistent_is_a_noop() {
        let q = make_queue();
        q.remove(999); // must not panic
        assert!(q.summary().is_none());
    }
}
