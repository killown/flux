//! Background task queue for tracking concurrent file I/O operations.
//!
//! Provides a thread-safe accumulator that decouples high-frequency progress
//! callbacks from the GTK main loop update rate.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A single in-flight background operation.
#[derive(Debug, Clone)]
pub struct Task {
    pub current: u64,
    pub total: u64,
    pub total_items: usize,
}

/// Shared, thread-safe registry of all active background operations.
#[derive(Debug, Default)]
pub struct TaskQueue {
    inner: Mutex<HashMap<u64, Task>>,
}

impl TaskQueue {
    /// Inserts or updates a task entry.
    pub fn update(&self, id: u64, current: u64, total: u64, total_items: usize) {
        if let Ok(mut map) = self.inner.lock() {
            map.insert(
                id,
                Task {
                    current,
                    total,
                    total_items,
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
