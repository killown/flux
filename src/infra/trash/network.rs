use super::recursive::delete_recursive;
use crate::domain::model::{AppMsg, FluxApp, NavigationMsg, WindowMsg};
use gtk::gio;
use gtk::gio::prelude::FileExt;
use relm4::prelude::*;

/// Deletes a network-resident path by walking it recursively on a worker
/// thread. `gio::File::trash` is not used here because remote filesystems
/// generally don't have a trash to move into.
pub(super) fn delete_via_recursive(file: gio::File, sender: AsyncComponentSender<FluxApp>) {
    crate::hit!("delete_via_recursive");
    eprintln!("[Delete] network path → skipping trash, using worker-thread delete_recursive");
    let uri = file.uri().to_string();
    relm4::spawn_blocking(move || {
        let file = gio::File::for_uri(&uri);
        match delete_recursive(&file) {
            Ok(()) => {
                sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
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
                    sender.input(AppMsg::Window(WindowMsg::ShowToast(
                        "Permission denied: Cannot delete item.".into(),
                    )));
                } else {
                    sender.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                        "Deletion error: {e}"
                    ))));
                }
            }
        }
    });
}
