use crate::model::{AppMsg, FluxApp};
use gtk::gdk;
use gtk::gio;
use relm4::prelude::*;

use super::util::{timestamped_name, unique_path};

impl FluxApp {
    /// Reads plain text from the clipboard and writes it to a `.txt` file.
    pub(super) fn paste_text(
        &self,
        clipboard: gdk::Clipboard,
        sender: &AsyncComponentSender<Self>,
    ) {
        let dest_dir = self.current_path.clone();
        let s = sender.clone();

        clipboard.read_text_async(None::<&gio::Cancellable>, move |res| {
            if let Ok(Some(text)) = res {
                let text = text.trim().to_string();
                if text.is_empty() {
                    return;
                }

                let file_name = timestamped_name("clipboard", "txt");
                let dest = unique_path(&dest_dir, &file_name);

                relm4::spawn_blocking(move || match std::fs::write(&dest, text.as_bytes()) {
                    Ok(()) => {
                        s.input(AppMsg::ShowToast(crate::i18n::tr("Text pasted as file")));
                        s.input(AppMsg::Refresh);
                    }
                    Err(e) => {
                        s.input(AppMsg::ShowToast(format!(
                            "{}: {}",
                            crate::i18n::tr("Failed to save text"),
                            e
                        )));
                    }
                });
            }
        });
    }
}
