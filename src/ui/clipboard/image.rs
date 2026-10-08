use crate::model::{AppMsg, FluxApp};
use gtk::gdk;
use gtk::gdk::prelude::TextureExt;
use gtk::gio;
use relm4::prelude::*;

use super::util::{timestamped_name, unique_path};

impl FluxApp {
    /// Reads an image buffer from the clipboard and writes it to the active directory.
    pub(super) fn paste_image(
        &self,
        clipboard: gdk::Clipboard,
        sender: &AsyncComponentSender<Self>,
    ) {
        let dest_dir = self.current_path.clone();
        let s = sender.clone();

        clipboard.read_texture_async(None::<&gio::Cancellable>, move |res| match res {
            Ok(Some(texture)) => {
                let file_name = timestamped_name("clipboard", "png");
                let dest = unique_path(&dest_dir, &file_name);

                match texture.save_to_png(dest.to_string_lossy().as_ref()) {
                    Ok(()) => {
                        s.input(AppMsg::ShowToast(crate::i18n::tr("Image pasted as file")));
                        s.input(AppMsg::Refresh);
                    }
                    Err(e) => {
                        s.input(AppMsg::ShowToast(format!(
                            "{}: {}",
                            crate::i18n::tr("Failed to save image"),
                            e
                        )));
                    }
                }
            }
            Ok(None) => {
                s.input(AppMsg::ShowToast(crate::i18n::tr(
                    "Could not read image from clipboard",
                )));
            }
            Err(e) => {
                s.input(AppMsg::ShowToast(format!(
                    "{}: {}",
                    crate::i18n::tr("Clipboard read error"),
                    e
                )));
            }
        });
    }
}
