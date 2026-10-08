use crate::model::{AppMsg, FluxApp};
use gtk::gdk;
use gtk::gio;
use relm4::prelude::*;

impl FluxApp {
    /// Handles standard URI-list paste operations.
    pub(super) fn paste_uri_list(
        &self,
        clipboard: gdk::Clipboard,
        sender: &AsyncComponentSender<Self>,
    ) {
        let s = sender.clone();
        let paste_toast = self
            .menu_actions
            .iter()
            .find(|a| a.command == "builtin::paste")
            .and_then(|a| a.toast.clone());

        clipboard.read_text_async(None::<&gio::Cancellable>, move |res| {
            if let Ok(Some(text)) = res {
                let mut lines = text.lines();
                let first_line = lines.next().unwrap_or("");
                let is_cut = first_line == "cut";

                let files: Vec<gio::File> = lines
                    .filter(|uri| !uri.is_empty())
                    .map(|uri| gio::File::for_uri(uri.trim_end_matches('\r')))
                    .collect();

                if !files.is_empty() {
                    s.input(AppMsg::PerformPaste { files, is_cut });
                    if let Some(toast) = paste_toast {
                        s.input(AppMsg::ShowToast(toast));
                    }
                }
            }
        });
    }
}
