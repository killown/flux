use super::speed::SpeedWindow;
use gtk::gio;
use std::time::Instant;

/// A single in-flight background operation.
#[derive(Debug, Clone)]
pub struct Task {
    /// The unique action name from `menu_actions` (e.g. `custom_0`), used to
    /// toggle `no_command_dialog`.
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
