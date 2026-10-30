use crate::domain::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

/// Loop-attaches, unlocks and mounts `path` off the async executor, reporting
/// the outcome as a `Result` so callers can tell a rejected passphrase apart.
async fn unlock_off_thread(path: PathBuf, passphrase: String) -> Result<PathBuf, String> {
    match relm4::spawn_blocking(move || {
        let image = crate::infra::luks::LuksImage { path };
        crate::infra::luks::unlock_and_mount(&image, &passphrase)
    })
    .await
    {
        Ok(result) => result,
        Err(e) => Err(format!("LUKS unlock task failed: {e}")),
    }
}

impl FluxApp {
    /// Unlocks `image_path` with the passphrase saved in the system keyring,
    /// falling back to the passphrase dialog when none is saved or it is rejected.
    pub fn handle_unlock_luks_image(
        &self,
        image_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_unlock_luks_image");
        let sender_c = sender.clone();
        relm4::spawn(async move {
            let Some(saved) = crate::infra::keyring::lookup_luks_passphrase(&image_path).await
            else {
                sender_c.input(AppMsg::PromptLuksPassphrase {
                    path: image_path,
                    wrong_passphrase: false,
                });
                return;
            };

            match unlock_off_thread(image_path.clone(), saved).await {
                Ok(mount_point) => sender_c.input(AppMsg::LuksMounted {
                    image_path,
                    mount_point,
                }),
                Err(e) if e == crate::i18n::tr("Wrong passphrase.") => {
                    sender_c.input(AppMsg::PromptLuksPassphrase {
                        path: image_path,
                        wrong_passphrase: true,
                    });
                }
                Err(e) => sender_c.input(AppMsg::ShowToast(e)),
            }
        });
    }

    /// Displays a modal passphrase entry dialog for an encrypted LUKS image.
    pub fn show_luks_passphrase_dialog(
        &self,
        image_path: PathBuf,
        wrong_passphrase: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("show_luks_passphrase_dialog");
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let title = if wrong_passphrase {
            crate::i18n::tr("Wrong Password")
        } else {
            crate::i18n::tr("Unlock LUKS Volume")
        };
        let body = if wrong_passphrase {
            crate::i18n::tr("The password you entered was incorrect. Please try again.")
        } else {
            crate::i18n::tr("Enter the passphrase to unlock this encrypted image.")
        };
        let dialog = adw::AlertDialog::new(Some(&title), Some(&body));

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

        let save_check =
            gtk::CheckButton::with_label(&crate::i18n::tr("Save password to the system keyring"));
        save_check.set_active(false);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.append(&entry);
        content.append(&save_check);

        dialog.set_extra_child(Some(&content));
        dialog.present(parent.as_ref());

        let entry_clone = entry.clone();
        let save_clone = save_check.clone();
        dialog.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let passphrase = entry_clone.text().to_string();
                if !passphrase.is_empty() {
                    let save_to_keyring = save_clone.is_active();
                    let path = image_path.clone();
                    let s = s.clone();
                    relm4::spawn(async move {
                        match unlock_off_thread(path.clone(), passphrase.clone()).await {
                            Ok(mount_point) => {
                                s.input(AppMsg::LuksMounted {
                                    image_path: path.clone(),
                                    mount_point,
                                });
                                if save_to_keyring {
                                    let saved = crate::infra::keyring::save_luks_passphrase(
                                        &path,
                                        &passphrase,
                                    )
                                    .await;
                                    let toast = if saved {
                                        crate::i18n::tr("Password saved to the system keyring")
                                    } else {
                                        crate::i18n::tr(
                                            "Could not save the password to the system keyring",
                                        )
                                    };
                                    s.input(AppMsg::ShowToast(toast));
                                }
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
