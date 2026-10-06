use std::sync::atomic::AtomicU64;

mod batch;
mod inner;
mod transfer;

pub use transfer::perform_file_op;

pub(crate) static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

/// Mutex to serialise conflict-resolution dialogs so only one is shown at a time.
static CONFLICT_MUTEX: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

/// Minimum number of files that triggers the dialog without a time delay.
const DIALOG_FILE_THRESHOLD: usize = 5;
/// Delay before showing the dialog for small copies that run longer than expected.
const DIALOG_DELAY: std::time::Duration = std::time::Duration::from_secs(2);
