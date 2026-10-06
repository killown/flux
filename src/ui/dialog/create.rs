use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Displays a modal dialog prompting for single or batch folder creation.
    pub fn show_prompt_new_folder(&self, sender: &AsyncComponentSender<Self>) {
        let window = gtk::Application::default().active_window();
        let current_path = self.current_path.clone();
        let s = sender.clone();

        let dialog = gtk::MessageDialog::new(
            window.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Other,
            gtk::ButtonsType::None,
            crate::i18n::tr("New Folder"),
        );
        dialog.set_secondary_text(Some(&crate::i18n::tr(
            "Enter folder name(s), separated by commas for batch creation:",
        )));
        dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
        let create_btn = dialog.add_button(&crate::i18n::tr("Create"), gtk::ResponseType::Ok);
        create_btn.style_context().add_class("suggested-action");
        dialog.set_default_response(gtk::ResponseType::Ok);

        let entry = gtk::Entry::builder()
            .text("New Folder")
            .activates_default(true)
            .build();
        entry.select_region(0, -1);
        entry.connect_map(|e| {
            e.grab_focus();
        });

        dialog.content_area().append(&entry);

        dialog.connect_response(move |dlg, response| {
            if response == gtk::ResponseType::Ok {
                let input = entry.text().to_string();
                let names: Vec<String> = input
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                if names.len() == 1 {
                    let name = &names[0];
                    let current_str = current_path.to_string_lossy();
                    if current_str.starts_with(crate::services::archive::ARCHIVE_URI)
                        || current_str.starts_with("/archive:/")
                        || current_str.starts_with("archive://")
                    {
                        if let Some((archive_path, prefix)) =
                            crate::services::archive::parse_archive_uri(&current_str)
                        {
                            let inner = if prefix.is_empty() {
                                name.to_string()
                            } else {
                                format!("{}/{}", prefix.trim_end_matches('/'), name)
                            };
                            match crate::services::archive::create_archive_directory(
                                &archive_path,
                                &inner,
                                None,
                            ) {
                                Ok(()) => s.input(AppMsg::Refresh),
                                Err(e) => s.input(AppMsg::ShowToast(format!("Archive error: {e}"))),
                            }
                        }
                    } else if crate::services::network::is_network_uri(&current_path) {
                        let uri = format!(
                            "{}/{}",
                            current_path.to_string_lossy().trim_end_matches('/'),
                            name
                        );
                        let s_clone = s.clone();
                        relm4::spawn_local(async move {
                            let file = gtk::gio::File::for_uri(&uri);
                            if file.query_exists(gtk::gio::Cancellable::NONE) {
                                s_clone.input(AppMsg::ShowToast(crate::i18n::tr(
                                    "Directory or file already exists",
                                )));
                                return;
                            }
                            if crate::services::network::create_network_directory(&uri, None)
                                .await
                                .is_ok()
                            {
                                s_clone.input(AppMsg::Navigate(PathBuf::from(uri)));
                            } else {
                                s_clone.input(AppMsg::ShowToast(crate::i18n::tr(
                                    "Failed to create directory",
                                )));
                            }
                        });
                    } else {
                        // existing local-filesystem branch unchanged
                        let folder_path = current_path.join(name);
                        if folder_path.exists() {
                            s.input(AppMsg::ShowToast(crate::i18n::tr(
                                "Directory or file already exists",
                            )));
                        } else if std::fs::create_dir(&folder_path).is_ok() {
                            s.input(AppMsg::InvalidateCacheAndNavigate(folder_path));
                        }
                    }
                } else if names.len() > 1 {
                    let mut created_count = 0;
                    let current_str = current_path.to_string_lossy();
                    let is_archive = current_str.starts_with(crate::services::archive::ARCHIVE_URI)
                        || current_str.starts_with("/archive:/")
                        || current_str.starts_with("archive://");
                    let parsed_archive = if is_archive {
                        crate::services::archive::parse_archive_uri(&current_str)
                    } else {
                        None
                    };
                    let is_network = crate::services::network::is_network_uri(&current_path);

                    for name in &names {
                        if is_archive {
                            if let Some((ref archive_path, ref prefix)) = parsed_archive {
                                let inner = if prefix.is_empty() {
                                    name.to_string()
                                } else {
                                    format!("{}/{}", prefix.trim_end_matches('/'), name)
                                };
                                if crate::services::archive::create_archive_directory(
                                    archive_path,
                                    &inner,
                                    None,
                                )
                                .is_ok()
                                {
                                    created_count += 1;
                                }
                            }
                        } else if is_network {
                            let uri = format!(
                                "{}/{}",
                                current_path.to_string_lossy().trim_end_matches('/'),
                                name
                            );
                            let file = gtk::gio::File::for_uri(&uri);
                            if !file.query_exists(gtk::gio::Cancellable::NONE)
                                && futures::executor::block_on(
                                    crate::services::network::create_network_directory(&uri, None),
                                )
                                .is_ok()
                            {
                                created_count += 1;
                            }
                        } else {
                            let folder_path = current_path.join(name);
                            if !folder_path.exists() && std::fs::create_dir(&folder_path).is_ok() {
                                created_count += 1;
                            }
                        }
                    }

                    s.input(AppMsg::Refresh);
                    s.input(AppMsg::ShowToast(format!(
                        "Created {} folders",
                        created_count
                    )));
                }
            }
            dlg.close();
        });

        dialog.present();
    }

    /// Displays a modal dialog prompting for single or batch file creation.
    pub fn show_prompt_new_file(&self, sender: &AsyncComponentSender<Self>) {
        let window = gtk::Application::default().active_window();
        let current_path = self.current_path.clone();
        let s = sender.clone();

        let dialog = gtk::MessageDialog::new(
            window.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Other,
            gtk::ButtonsType::None,
            crate::i18n::tr("New File"),
        );
        dialog.set_secondary_text(Some(&crate::i18n::tr(
            "Enter file name(s), separated by commas for batch creation:",
        )));
        dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
        let create_btn = dialog.add_button(&crate::i18n::tr("Create"), gtk::ResponseType::Ok);
        create_btn.style_context().add_class("suggested-action");
        dialog.set_default_response(gtk::ResponseType::Ok);

        let entry = gtk::Entry::builder()
            .text("new_file.txt")
            .activates_default(true)
            .build();
        entry.select_region(0, -1);
        entry.connect_map(|e| {
            e.grab_focus();
        });

        dialog.content_area().append(&entry);

        dialog.connect_response(move |dlg, response| {
            if response == gtk::ResponseType::Ok {
                let input = entry.text().to_string();
                let names: Vec<String> = input
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                if names.len() == 1 {
                    let name = &names[0];
                    if crate::services::network::is_network_uri(&current_path) {
                        let uri = format!(
                            "{}/{}",
                            current_path.to_string_lossy().trim_end_matches('/'),
                            name
                        );
                        let file = gtk::gio::File::for_uri(&uri);
                        if file.query_exists(gtk::gio::Cancellable::NONE) {
                            s.input(AppMsg::ShowToast(crate::i18n::tr(
                                "Directory or file already exists",
                            )));
                        } else if let Ok(stream) = file
                            .create(gtk::gio::FileCreateFlags::NONE, gtk::gio::Cancellable::NONE)
                        {
                            let _ = stream.close(gtk::gio::Cancellable::NONE);
                            Self::open_file(PathBuf::from(uri));
                            s.input(AppMsg::Refresh);
                        }
                    } else {
                        let file_path = current_path.join(name);
                        if file_path.exists() {
                            s.input(AppMsg::ShowToast(crate::i18n::tr(
                                "Directory or file already exists",
                            )));
                        } else if std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&file_path)
                            .is_ok()
                        {
                            Self::open_file(file_path);
                            s.input(AppMsg::Refresh);
                        }
                    }
                } else if names.len() > 1 {
                    let mut created_count = 0;
                    let is_network = crate::services::network::is_network_uri(&current_path);

                    for name in &names {
                        if is_network {
                            let uri = format!(
                                "{}/{}",
                                current_path.to_string_lossy().trim_end_matches('/'),
                                name
                            );
                            let file = gtk::gio::File::for_uri(&uri);
                            if !file.query_exists(gtk::gio::Cancellable::NONE) {
                                if let Ok(stream) = file.create(
                                    gtk::gio::FileCreateFlags::NONE,
                                    gtk::gio::Cancellable::NONE,
                                ) {
                                    let _ = stream.close(gtk::gio::Cancellable::NONE);
                                    created_count += 1;
                                }
                            }
                        } else {
                            let file_path = current_path.join(name);
                            if !file_path.exists()
                                && std::fs::OpenOptions::new()
                                    .write(true)
                                    .create_new(true)
                                    .open(&file_path)
                                    .is_ok()
                            {
                                created_count += 1;
                            }
                        }
                    }

                    s.input(AppMsg::Refresh);
                    s.input(AppMsg::ShowToast(format!(
                        "Created {} files",
                        created_count
                    )));
                }
            }
            dlg.close();
        });

        dialog.present();
    }
}
