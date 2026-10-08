use crate::model::{AppMsg, FluxApp};
use gtk::gdk;
use gtk::gdk::prelude::DisplayExt;
use relm4::prelude::*;

impl FluxApp {
    /// Inspects the GDK clipboard and dispatches the appropriate paste variant.
    pub fn handle_paste_from_clipboard(&self, sender: &AsyncComponentSender<Self>) {
        let Some(display) = gdk::Display::default() else {
            sender.input(AppMsg::ShowToast(crate::i18n::tr(
                "No display available for clipboard operation",
            )));
            return;
        };

        let clipboard = display.clipboard();
        let formats = clipboard.formats();

        let has_uri_list = formats.contain_mime_type("text/uri-list");
        if has_uri_list {
            self.paste_uri_list(clipboard, sender);
            return;
        }

        let has_png = formats.contain_mime_type("image/png");
        let has_jpeg =
            formats.contain_mime_type("image/jpeg") || formats.contain_mime_type("image/jpg");

        if has_png || has_jpeg {
            self.paste_image(clipboard, sender);
            return;
        }

        if formats.contain_mime_type("text/html") {
            self.paste_html(clipboard, sender);
            return;
        }

        if formats.contain_mime_type("text/plain")
            || formats.contain_mime_type("text/plain;charset=utf-8")
        {
            self.paste_text(clipboard, sender);
            return;
        }

        sender.input(AppMsg::ShowToast(crate::i18n::tr(
            "Nothing pasteable in clipboard",
        )));
    }
}
