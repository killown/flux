use super::recursive::delete_recursive;
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
                    "[Delete] trash_async failed gio_kind={:?} msg={:?}, falling back to delete_recursive",
                    trash_err.kind::<gio::IOErrorEnum>(),
                    trash_err.message()
                );

                let s_inner = sender.clone();
                relm4::spawn_blocking(move || {
                    match delete_recursive(&file_for_fallback) {
                        Ok(()) => {
                            s_inner.input(AppMsg::Refresh);
                        }
                        Err(e) => {
                            eprintln!(
                                "[Delete] fallback delete_recursive failed gio_kind={:?} msg={:?}",
                                e.kind::<gio::IOErrorEnum>(),
                                e.message()
                            );
                            if e.message().contains("Permission denied")
                                || e.message().contains("Operation not permitted")
                            {
                                s_inner.input(AppMsg::ShowToast(
                                    "Permission denied: Cannot delete item.".into(),
                                ));
                            } else {
                                s_inner.input(AppMsg::ShowToast(format!(
                                    "Deletion error: {}",
                                    e
                                )));
                            }
                        }
                    }
                });
            }
        },
    );
}
