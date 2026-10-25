use super::tracked::trash_tracked;
use crate::model::{AppMsg, FluxApp};
use gtk::gio;
use gtk::gio::prelude::FileExt;
use relm4::prelude::*;
use std::path::PathBuf;

/// Sends the file to the trash on a worker thread and reports which trash
/// entry the deletion created, so undo can restore that exact entry.
///
/// On failure a toast is shown and nothing is recorded in the undo history.
pub(super) fn delete_via_trash(
    file: gio::File,
    raw_path: PathBuf,
    sender: AsyncComponentSender<FluxApp>,
) {
    // `gio::File` is not `Send`, so hand the worker thread the URI instead.
    let uri = file.uri().to_string();

    relm4::spawn_blocking(move || {
        let file = gio::File::for_uri(&uri);
        match trash_tracked(&file, &raw_path) {
            Ok(item) => {
                sender.input(AppMsg::TrashSucceeded(vec![item]));
                sender.input(AppMsg::Refresh);
            }
            Err(trash_err) => {
                eprintln!(
                    "[Delete] trash failed gio_kind={:?} msg={:?}",
                    trash_err.kind::<gio::IOErrorEnum>(),
                    trash_err.message()
                );
                sender.input(AppMsg::ShowToast(format!(
                    "{}: {}",
                    crate::i18n::tr("Could not move to trash"),
                    trash_err.message()
                )));
            }
        }
    });
}
