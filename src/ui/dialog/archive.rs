use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;
use std::path::Path;
use std::path::PathBuf;

impl FluxApp {
    /// Displays a modal password entry dialog for encrypted archives.
    pub fn show_prompt_archive_password(
        &mut self,
        archive_path: PathBuf,
        prefix: String,
        wrong_password: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.archive_locked = true;
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let title = if wrong_password {
            crate::i18n::tr("Wrong Password")
        } else {
            crate::i18n::tr("Archive is password-protected")
        };

        let dialog = gtk::MessageDialog::new(
            parent.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Question,
            gtk::ButtonsType::None,
            &title,
        );

        let secondary = if wrong_password {
            crate::i18n::tr("The password you entered was incorrect. Please try again.")
        } else {
            crate::i18n::tr("This archive is encrypted. Enter the password to browse its contents.")
        };
        dialog.set_secondary_text(Some(&secondary));

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
                let password = entry_clone.text().to_string();
                if !password.is_empty() {
                    s.input(AppMsg::LoadArchiveWithPassword {
                        archive_path: archive_path.clone(),
                        prefix: prefix.clone(),
                        password,
                    });
                }
            }
            dlg.close();
        });
    }

    pub fn show_archive_deletion_warning(
        &self,
        archive_path: PathBuf,
        inner_path: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let file_name = Path::new(&inner_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&inner_path)
            .to_string();

        let title = crate::i18n::tr("Permanently delete \"{}\"?").replace("{}", &file_name);
        let dialog = gtk::MessageDialog::new(
            parent.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Warning,
            gtk::ButtonsType::None,
            &title,
        );

        dialog.set_secondary_text(Some(&crate::i18n::tr(
            "This item will be permanently deleted from the archive. This action is irreversible and cannot be undone.",
        )));

        dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
        let delete_btn = dialog.add_button(&crate::i18n::tr("Delete"), gtk::ResponseType::Ok);
        delete_btn.style_context().add_class("destructive-action");
        dialog.set_default_response(gtk::ResponseType::Cancel);

        dialog.connect_response(move |dlg, resp| {
            if resp == gtk::ResponseType::Ok {
                let s_clone = s.clone();
                let a_path = archive_path.clone();
                let i_path = inner_path.clone();
                let file_name_clone = file_name.clone();

                relm4::spawn_blocking(move || {
                    let total_bytes = std::fs::metadata(&a_path).map(|m| m.len()).unwrap_or(0);
                    let show_progress = total_bytes >= 100 * 1024 * 1024;
                    let task_id = crate::ui::paste_ops::NEXT_TASK_ID
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let cancellable = gtk::gio::Cancellable::new();
                    let label = format!("Deleting {}", file_name_clone);

                    if show_progress {
                        s_clone.input(AppMsg::TaskProgress {
                            id: task_id,
                            label: label.clone(),
                            current: 0,
                            total: total_bytes,
                            total_items: 1,
                            cancellable: cancellable.clone(),
                        });
                        s_clone.input(AppMsg::TaskQueueTick);
                    }

                    let mut copied_bytes = 0u64;
                    let s_cb = s_clone.clone();
                    let label_cb = label.clone();
                    let cancellable_cb = cancellable.clone();

                    let mut progress_cb = move |chunk: u64| {
                        copied_bytes += chunk;
                        if show_progress {
                            s_cb.input(AppMsg::TaskProgress {
                                id: task_id,
                                label: label_cb.clone(),
                                current: copied_bytes.min(total_bytes),
                                total: total_bytes,
                                total_items: 1,
                                cancellable: cancellable_cb.clone(),
                            });
                            s_cb.input(AppMsg::TaskQueueTick);
                        }
                    };

                    let result = crate::services::archive::remove_archive_entry(
                        &a_path,
                        &i_path,
                        None,
                        Some(&mut progress_cb),
                    );

                    if show_progress {
                        s_clone.input(AppMsg::TaskCompleted(task_id));
                        s_clone.input(AppMsg::TaskQueueTick);
                    }

                    match result {
                        Ok(()) => {
                            s_clone.input(AppMsg::Refresh);
                        }
                        Err(e) => {
                            s_clone.input(AppMsg::ShowToast(format!("Archive error: {e}")));
                        }
                    }
                });
            }
            dlg.close();
        });

        dialog.present();
    }
}
