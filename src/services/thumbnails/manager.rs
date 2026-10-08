use super::permits::effective_thumbnail_permits;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Semaphore;

/// Tracks active lazy thumbnail generation tasks to allow viewport-based cancellation.
#[derive(Default, Debug)]
pub struct ThumbnailTaskManager {
    active_tokens: Mutex<HashMap<u32, Arc<AtomicBool>>>,
    semaphore: OnceLock<Arc<Semaphore>>,
}

impl ThumbnailTaskManager {
    /// Returns the shared semaphore matching the configured worker thread limit,
    /// clamped to the process's FD budget.
    pub fn get_semaphore(&self, max_threads: usize) -> Arc<Semaphore> {
        let permits = effective_thumbnail_permits(max_threads);
        self.semaphore
            .get_or_init(|| Arc::new(Semaphore::new(permits)))
            .clone()
    }

    /// Cancels tasks for grid indices that are no longer within the visible viewport bounds.
    /// Returns the list of cancelled indices so caller can evict them from pending tracking.
    pub fn cancel_out_of_viewport(&self, visible_start: u32, visible_end: u32) -> Vec<u32> {
        let mut cancelled = Vec::new();
        let mut tokens = self.active_tokens.lock().unwrap();
        tokens.retain(|&idx, cancel_flag| {
            if idx < visible_start || idx > visible_end {
                cancel_flag.store(true, Ordering::Release);
                cancelled.push(idx);
                false
            } else {
                true
            }
        });
        cancelled
    }

    /// Registers a cancellation flag for a newly requested thumbnail index.
    pub fn register_token(&self, grid_idx: u32) -> Arc<AtomicBool> {
        let mut tokens = self.active_tokens.lock().unwrap();
        if let Some(old) = tokens.remove(&grid_idx) {
            old.store(true, Ordering::Release);
        }
        let flag = Arc::new(AtomicBool::new(false));
        tokens.insert(grid_idx, flag.clone());
        flag
    }

    /// Clears task tracking on completion.
    pub fn complete_task(&self, grid_idx: u32) {
        if let Ok(mut tokens) = self.active_tokens.lock() {
            tokens.remove(&grid_idx);
        }
    }

    /// Cancels all ongoing tasks (e.g. during directory changes).
    pub fn clear_and_cancel_all(&self) {
        if let Ok(mut tokens) = self.active_tokens.lock() {
            for (_, flag) in tokens.drain() {
                flag.store(true, Ordering::Release);
            }
        }
    }
}
