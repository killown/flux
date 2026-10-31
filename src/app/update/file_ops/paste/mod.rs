mod batch;
mod inner;
mod transfer;

#[allow(unused_imports)]
pub use transfer::{
    archive_inner_path, clean_tmp_basename, is_same_entry, paste_name_for, perform_file_op,
    remove_cut_source,
};

/// Mutex to serialise conflict-resolution dialogs so only one is shown at a time.
static CONFLICT_MUTEX: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

/// Minimum number of files that triggers the dialog without a time delay.
const DIALOG_FILE_THRESHOLD: usize = 5;
/// Delay before showing the dialog for small copies that run longer than expected.
const DIALOG_DELAY: std::time::Duration = std::time::Duration::from_secs(2);
