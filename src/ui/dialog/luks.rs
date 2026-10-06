use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn show_luks_passphrase_dialog(
        &self,
        image_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let dialog = gtk::MessageDialog::new(
            parent.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Question,
            gtk::ButtonsType::None,
            crate::i18n::tr("Unlock LUKS Volume"),
        );
        dialog.set_secondary_text(Some(&crate::i18n::tr(
            "Enter the passphrase to unlock this encrypted image.",
        )));

        dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
        let unlock_btn = dialog.add_button(&crate::i18n::tr("Unlock"), gtk::ResponseType::Ok);
        unlock_btn.style_context().add_class("suggested-action");
        dialog.set_default_response(gtk::ResponseType::Ok);

        let entry = gtk::PasswordEntry::builder()
            .show_peek_icon(true)
            .activates_default(true)
            .margin_top(8)
            .margin_bottom(4)
            .margin_start(16)
            .margin_end(16)
            .build();

        entry.connect_map(|e| {
            e.grab_focus();
        });

        dialog.content_area().append(&entry);
        dialog.present();

        let entry_clone = entry.clone();
        dialog.connect_response(move |dlg, resp| {
            if resp == gtk::ResponseType::Ok {
                let passphrase = entry_clone.text().to_string();
                if !passphrase.is_empty() {
                    let path = image_path.clone();
                    let s = s.clone();
                    relm4::spawn_blocking(move || {
                        let image = crate::services::luks::LuksImage { path: path.clone() };
                        match crate::services::luks::unlock_and_mount(&image, &passphrase) {
                            Ok(mount_point) => {
                                s.input(AppMsg::LuksMounted {
                                    image_path: path,
                                    mount_point,
                                });
                            }
                            Err(e) => {
                                s.input(AppMsg::ShowToast(e));
                            }
                        }
                    });
                }
            }
            dlg.close();
        });
    }
}
