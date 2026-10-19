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

        let dialog = adw::AlertDialog::new(
            Some(&crate::i18n::tr("Unlock LUKS Volume")),
            Some(&crate::i18n::tr(
                "Enter the passphrase to unlock this encrypted image.",
            )),
        );

        dialog.add_response("cancel", &crate::i18n::tr("Cancel"));
        dialog.add_response("ok", &crate::i18n::tr("Unlock"));
        dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("cancel");

        let entry = gtk::PasswordEntry::builder()
            .show_peek_icon(true)
            .activates_default(true)
            .build();

        entry.connect_map(|e| {
            e.grab_focus();
        });

        dialog.set_extra_child(Some(&entry));
        dialog.present(parent.as_ref());

        let entry_clone = entry.clone();
        dialog.connect_response(None, move |_, resp| {
            if resp == "ok" {
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
        });
    }
}
