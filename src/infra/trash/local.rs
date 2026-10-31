use super::tracked::trash_tracked_batch;
use crate::domain::model::TrashedItem;
use gtk::gio;
use gtk::glib;
use std::path::PathBuf;

/// A file that could not be moved to the trash.
#[derive(Debug)]
pub struct TrashFailure {
    /// Path the item is known by in the app.
    #[allow(dead_code)]
    pub path: PathBuf,
    /// Error reported by GIO.
    pub error: glib::Error,
}

/// Sends every file to the trash and reports which trash entries the deletion
/// created, so undo can restore those exact entries.
///
/// Blocking: call it from a worker thread. `targets` holds `(uri, original
/// path)` pairs. The selection is one batch, so the trash is listed once for
/// all of it and the whole thing is a single undo step. A file that cannot be
/// trashed is returned in the failure list and does not stop the others, only
/// the files that were trashed are returned as [`TrashedItem`]s.
pub fn trash_local_batch(targets: Vec<(String, PathBuf)>) -> (Vec<TrashedItem>, Vec<TrashFailure>) {
    crate::hit!("trash_local_batch");
    let files: Vec<(gio::File, PathBuf)> = targets
        .into_iter()
        .map(|(uri, raw_path)| (gio::File::for_uri(&uri), raw_path))
        .collect();

    let (trashed, failed) = trash_tracked_batch(&files);

    let failures = failed
        .into_iter()
        .map(|(idx, error)| {
            eprintln!(
                "[Delete] trash failed path={:?} gio_kind={:?} msg={:?}",
                files[idx].1,
                error.kind::<gio::IOErrorEnum>(),
                error.message()
            );
            TrashFailure {
                path: files[idx].1.clone(),
                error,
            }
        })
        .collect();

    (trashed, failures)
}
