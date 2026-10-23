use crate::model::{AppMsg, FluxApp};
use gtk::gio::prelude::FileExt;
use gtk::{gio, glib};
use relm4::prelude::*;
use std::path::PathBuf;

/// Sends the file to the trash, falling back to a permanent recursive delete
/// if the trash operation fails (e.g. the file is on a filesystem without a
/// trash directory, or is inside a mount point where trash is unavailable).
pub(super) fn delete_via_trash(
    file: gio::File,
    raw_path: PathBuf,
    sender: AsyncComponentSender<FluxApp>,
) {
    let file_for_fallback = file.clone();
    let trashed_path = raw_path;

    file.trash_async(
        glib::Priority::DEFAULT,
        gio::Cancellable::NONE,
        move |res| match res {
            Ok(_) => {
                sender.input(AppMsg::TrashSucceeded(vec![trashed_path]));
                sender.input(AppMsg::Refresh);
            }
            Err(trash_err) => {
                eprintln!(
                    "[Delete] trash_async failed gio_kind={:?} msg={:?}",
                    trash_err.kind::<gio::IOErrorEnum>(),
                    trash_err.message()
                );
                let _ = file_for_fallback;
                sender.input(AppMsg::ShowToast(format!(
                    "{}: {}",
                    crate::i18n::tr("Could not move to trash"),
                    trash_err.message()
                )));
            }
        },
    );
}
