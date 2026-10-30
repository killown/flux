use super::tracked::trash_tracked_batch;
use crate::domain::model::{AppMsg, FileOpsMsg, NavigationMsg, WindowMsg, FluxApp};
use gtk::gio;
use gtk::gio::prelude::FileExt;
use relm4::prelude::*;
use std::path::PathBuf;

/// Sends every file to the trash on one worker thread and reports which trash
/// entries the deletion created, so undo can restore those exact entries.
///
/// The selection is one batch, so the trash is listed once for all of it and
/// the whole thing is a single undo step. A file that cannot be trashed gets a
/// toast and does not stop the others, only the files that were trashed are
/// recorded in the undo history.
pub(super) fn delete_batch_via_trash(
    items: Vec<(gio::File, PathBuf)>,
    sender: AsyncComponentSender<FluxApp>,
) {
    crate::hit!("delete_batch_via_trash");
    // `gio::File` is not `Send`, so hand the worker thread the URIs instead.
    let targets: Vec<(String, PathBuf)> = items
        .into_iter()
        .map(|(file, raw_path)| (file.uri().to_string(), raw_path))
        .collect();

    relm4::spawn_blocking(move || {
        let files: Vec<(gio::File, PathBuf)> = targets
            .into_iter()
            .map(|(uri, raw_path)| (gio::File::for_uri(&uri), raw_path))
            .collect();

        let (trashed, failed) = trash_tracked_batch(&files);

        for (idx, trash_err) in failed {
            eprintln!(
                "[Delete] trash failed path={:?} gio_kind={:?} msg={:?}",
                files[idx].1,
                trash_err.kind::<gio::IOErrorEnum>(),
                trash_err.message()
            );
            sender.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                "{}: {}",
                crate::i18n::tr("Could not move to trash"),
                trash_err.message()
            ))));
        }

        if !trashed.is_empty() {
            sender.input(AppMsg::FileOps(FileOpsMsg::TrashSucceeded(trashed)));
            sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
        }
    });
}
