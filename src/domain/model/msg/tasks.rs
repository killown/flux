//! Background task queue.

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TasksMsg {
    /// Report progress for a specific background operation slot.
    TaskProgress {
        id: u64,
        /// Human-readable description (filename or "N files").
        label: String,
        current: u64,
        total: u64,
        total_items: usize,
        cancellable: gio::Cancellable,
    },
    /// Signal that a specific background operation has completed.
    TaskCompleted(u64),
    #[allow(dead_code)]
    /// Cancel a single in-flight background operation by its task ID.
    CancelTask(u64),
    /// Cancel every in-flight background operation immediately.
    CancelAllTasks,
    /// Throttled tick to refresh the status bar from the task queue.
    TaskQueueTick,
}
