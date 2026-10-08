use crate::model::{AppMsg, FluxApp};
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use relm4::prelude::*;

use super::stream::read_stream_async_into_bytes;

impl FluxApp {
    /// Reads HTML markup from the clipboard and writes it as a `.html` file.
    pub(super) fn paste_html(
        &self,
        clipboard: gdk::Clipboard,
        sender: &AsyncComponentSender<Self>,
    ) {
        let dest_dir = self.current_path.clone();
        let s = sender.clone();

        clipboard.read_async(
            &["text/html"],
            glib::Priority::DEFAULT,
            None::<&gio::Cancellable>,
            move |res| match res {
                Ok((stream, _actual_mime)) => {
                    read_stream_async_into_bytes(stream, dest_dir, s);
                }
                Err(e) => {
                    s.input(AppMsg::ShowToast(format!(
                        "{}: {}",
                        crate::i18n::tr("Clipboard read error"),
                        e
                    )));
                }
            },
        );
    }
}
