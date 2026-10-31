use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, ArchiveMsg, NavigationMsg, TasksMsg, WindowMsg};
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
        crate::hit!("show_prompt_archive_password");
        self.archive.locked = true;
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let title = if wrong_password {
            crate::i18n::tr("Wrong Password")
        } else {
            crate::i18n::tr("Archive is password-protected")
        };

        let dialog = adw::AlertDialog::new(Some(&title), None);

        let secondary = if wrong_password {
            crate::i18n::tr("The password you entered was incorrect. Please try again.")
        } else {
            crate::i18n::tr("This archive is encrypted. Enter the password to browse its contents.")
        };
        dialog.set_body(&secondary);

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
                let password = entry_clone.text().to_string();
                if !password.is_empty() {
                    let save_to_keyring = save_clone.is_active();
                    s.input(AppMsg::Archive(ArchiveMsg::LoadArchiveWithPassword {
                        archive_path: archive_path.clone(),
                        prefix: prefix.clone(),
                        password: password.clone(),
                    }));
                    if save_to_keyring {
                        s.input(AppMsg::Archive(ArchiveMsg::SaveArchivePasswordToKeyring {
                            archive_path: archive_path.clone(),
                            password,
                        }));
                    }
                }
            }
        });
    }

    /// Asks whether to extract an archive nested inside another archive.
    pub fn show_nested_archive_prompt(
        &self,
        archive_path: PathBuf,
        inner_path: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("show_nested_archive_prompt");
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let name_of = |p: &Path| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string_lossy().into_owned())
        };
        let inner_name = name_of(Path::new(&inner_path));
        let outer_name = name_of(&archive_path);

        let title = crate::i18n::tr("Extract \"{}\"?").replace("{}", &inner_name);
        let body = crate::i18n::tr(
            "\"{}\" is an archive inside \"{}\". It will be extracted next to \"{}\" and then opened.",
        )
        .replacen("{}", &inner_name, 1)
        .replacen("{}", &outer_name, 1)
        .replacen("{}", &outer_name, 1);

        let dialog = adw::AlertDialog::new(Some(&title), Some(&body));
        dialog.add_response("no", &crate::i18n::tr("No"));
        dialog.add_response("yes", &crate::i18n::tr("Yes"));
        dialog.set_response_appearance("yes", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("yes"));
        dialog.set_close_response("no");

        dialog.connect_response(None, move |_, resp| {
            if resp == "yes" {
                s.input(AppMsg::Archive(ArchiveMsg::ExtractNestedArchive {
                    archive_path: archive_path.clone(),
                    inner_path: inner_path.clone(),
                }));
            }
        });

        dialog.present(parent.as_ref());
    }

    pub fn show_archive_deletion_warning(
        &self,
        archive_path: PathBuf,
        inner_path: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("show_archive_deletion_warning");
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();
        let password = self.archive.passwords.get(&archive_path).cloned();

        let file_name = Path::new(&inner_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&inner_path)
            .to_string();

        let title = crate::i18n::tr("Permanently delete \"{}\"?").replace("{}", &file_name);
        let dialog = adw::AlertDialog::new(
            Some(&title),
            Some(&crate::i18n::tr(
                "This item will be permanently deleted from the archive. This action is irreversible and cannot be undone.",
            )),
        );

        dialog.add_response("cancel", &crate::i18n::tr("Cancel"));
        dialog.add_response("delete", &crate::i18n::tr("Delete"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        dialog.connect_response(None, move |_, resp| {
            if resp == "delete" {
                let s_clone = s.clone();
                let a_path = archive_path.clone();
                let i_path = inner_path.clone();
                let file_name_clone = file_name.clone();
                let password = password.clone();

                relm4::spawn_blocking(move || {
                    let total_bytes = std::fs::metadata(&a_path).map(|m| m.len()).unwrap_or(0);
                    let show_progress = total_bytes >= 100 * 1024 * 1024;
                    let task_id = crate::infra::tasks::NEXT_TASK_ID
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let cancellable = gtk::gio::Cancellable::new();
                    let label = format!("Deleting {}", file_name_clone);

                    if show_progress {
                        s_clone.input(AppMsg::Tasks(TasksMsg::TaskProgress {
                            id: task_id,
                            label: label.clone(),
                            current: 0,
                            total: total_bytes,
                            total_items: 1,
                            cancellable: cancellable.clone().into(),
                        }));
                        s_clone.input(AppMsg::Tasks(TasksMsg::TaskQueueTick));
                    }

                    let mut copied_bytes = 0u64;
                    let s_cb = s_clone.clone();
                    let label_cb = label.clone();
                    let cancellable_cb = cancellable.clone();

                    let mut progress_cb = move |chunk: u64| {
                        copied_bytes += chunk;
                        if show_progress {
                            s_cb.input(AppMsg::Tasks(TasksMsg::TaskProgress {
                                id: task_id,
                                label: label_cb.clone(),
                                current: copied_bytes.min(total_bytes),
                                total: total_bytes,
                                total_items: 1,
                                cancellable: cancellable_cb.clone().into(),
                            }));
                            s_cb.input(AppMsg::Tasks(TasksMsg::TaskQueueTick));
                        }
                    };

                    let was_encrypted = crate::infra::archive::zip_is_encrypted(&a_path);

                    let result = crate::infra::archive::remove_archive_entry(
                        &a_path,
                        &i_path,
                        password.as_deref(),
                        Some(&mut progress_cb),
                    );

                    if show_progress {
                        s_clone.input(AppMsg::Tasks(TasksMsg::TaskCompleted(task_id)));
                        s_clone.input(AppMsg::Tasks(TasksMsg::TaskQueueTick));
                    }

                    match result {
                        Ok(()) => {
                            s_clone.input(AppMsg::Navigation(NavigationMsg::Refresh));
                            if was_encrypted && !crate::infra::archive::zip_is_encrypted(&a_path)
                            {
                                s_clone.input(AppMsg::Window(WindowMsg::ShowToast(crate::i18n::tr(
                                    "No encrypted files left: this archive is no longer password-protected, and files you add next will not be encrypted.",
                                ))));
                            }
                        }
                        Err(e) => {
                            s_clone.input(AppMsg::Window(WindowMsg::ShowToast(format!("Archive error: {e}"))));
                        }
                    }
                });
            }
        });

        dialog.present(parent.as_ref());
    }
}
