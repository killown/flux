use super::recursive::delete_recursive;
use crate::model::{AppMsg, FluxApp};
use gtk::gio::prelude::FileExt;
use gtk::{gio, glib};
use relm4::prelude::*;

/// Deletes a network-resident path by walking it recursively on the GLib main
/// context. `gio::File::trash` is not used here because remote filesystems
/// generally don't have a trash to move into.
pub(super) fn delete_via_recursive(file: gio::File, sender: AsyncComponentSender<FluxApp>) {
    eprintln!("[Delete] network path → skipping trash, using GLib-context delete_recursive");
    glib::MainContext::default().spawn_local(async move {
        match delete_recursive(&file) {
            Ok(()) => {
                sender.input(AppMsg::Refresh);
            }
            Err(e) => {
                eprintln!(
                    "[Delete] delete_recursive failed uri={:?} gio_kind={:?} msg={:?}",
                    file.uri().to_string(),
                    e.kind::<gio::IOErrorEnum>(),
                    e.message()
                );
                if e.message().contains("Permission denied")
                    || e.message().contains("Operation not permitted")
                {
                    sender.input(AppMsg::ShowToast(
                        "Permission denied: Cannot delete item.".into(),
                    ));
                } else {
                    sender.input(AppMsg::ShowToast(format!("Deletion error: {e}")));
                }
            }
        }
    });
}
