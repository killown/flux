use crate::model::{AppMsg, FluxApp};
use adw::gdk;
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
                        } else if crate::services::network::create_network_directory(&uri, None)
                            .is_ok()
                        {
                            s.input(AppMsg::Navigate(PathBuf::from(uri)));
                        }
                    } else {
                        let folder_path = current_path.join(name);
                        if folder_path.exists() {
                            s.input(AppMsg::ShowToast(crate::i18n::tr(
                                "Directory or file already exists",
                            )));
                        } else if std::fs::create_dir(&folder_path).is_ok() {
                            s.input(AppMsg::Navigate(folder_path));
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
                            if !file.query_exists(gtk::gio::Cancellable::NONE)
                                && crate::services::network::create_network_directory(&uri, None)
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
                            crate::utils::open_file(PathBuf::from(uri));
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
                            crate::utils::open_file(file_path);
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

    /// Displays a modal prompt to rename a sidebar bookmark.
    pub fn show_prompt_sidebar_rename(
        &self,
        target_path: PathBuf,
        current_name: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let dialog = gtk::MessageDialog::new(
            parent.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Other,
            gtk::ButtonsType::None,
            crate::i18n::tr("Rename Bookmark"),
        );
        dialog.set_secondary_text(Some(&crate::i18n::tr(
            "Enter a new display name for this bookmark:",
        )));

        dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
        let rename_btn = dialog.add_button(&crate::i18n::tr("Rename"), gtk::ResponseType::Ok);
        rename_btn.style_context().add_class("suggested-action");
        dialog.set_default_response(gtk::ResponseType::Ok);

        let entry = gtk::Entry::builder()
            .text(&current_name)
            .activates_default(true)
            .margin_top(8)
            .margin_bottom(4)
            .margin_start(16)
            .margin_end(16)
            .build();

        entry.select_region(0, -1);
        entry.connect_map(|e| {
            e.grab_focus();
        });

        dialog.content_area().append(&entry);
        dialog.present();

        let entry_clone = entry.clone();
        dialog.connect_response(move |dlg, resp| {
            if resp == gtk::ResponseType::Ok {
                let new_name = entry_clone.text().trim().to_string();
                if !new_name.is_empty() {
                    s.input(AppMsg::RenameSidebarPlace {
                        path: target_path.clone(),
                        new_name,
                    });
                }
            }
            dlg.close();
        });
    }

    /// Displays a symbolic-only icon picker dialog for sidebar locations.
    pub fn show_sidebar_icon_picker(
        &self,
        target_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let toplevels = gtk::Window::list_toplevels();
        let parent = toplevels
            .first()
            .and_then(|w| w.downcast_ref::<gtk::Window>());
        let dialog = gtk::Dialog::builder()
            .title(crate::i18n::tr("Select Sidebar Icon"))
            .transient_for(parent.unwrap())
            .modal(true)
            .use_header_bar(1)
            .build();
        let flow_box = gtk::FlowBox::builder()
            .valign(gtk::Align::Start)
            .max_children_per_line(8)
            .min_children_per_line(8)
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&flow_box)
            .height_request(380)
            .width_request(480)
            .build();
        let search_entry = gtk::SearchEntry::builder()
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        let content_area = dialog.content_area();
        content_area.append(&search_entry);
        content_area.append(&scrolled);

        let icon_theme = gtk::IconTheme::for_display(&gdk::Display::default().unwrap());
        let icon_names = icon_theme.icon_names();

        for icon_name in icon_names {
            let name_str = icon_name.as_str();
            if name_str.ends_with("-symbolic") {
                let image = gtk::Image::from_icon_name(name_str);
                image.set_pixel_size(20);
                let button = gtk::Button::builder()
                    .child(&image)
                    .tooltip_text(name_str)
                    .has_frame(false)
                    .build();
                unsafe {
                    button.set_data("icon-name", icon_name.to_string());
                }
                let dialog_btn_clone = dialog.clone();
                let flow_box_btn_clone = flow_box.clone();
                button.connect_clicked(move |btn| {
                    if let Some(row) = btn
                        .parent()
                        .and_then(|p| p.downcast::<gtk::FlowBoxChild>().ok())
                    {
                        flow_box_btn_clone.select_child(&row);
                        dialog_btn_clone.response(gtk::ResponseType::Ok);
                    }
                });
                flow_box.append(&button);
            }
        }

        let flow_box_clone = flow_box.clone();
        search_entry.connect_search_changed(move |entry| {
            let text = entry.text().to_string().to_lowercase();
            let mut child = flow_box_clone.first_child();
            while let Some(ref widget) = child {
                if let Some(child_row) = widget.downcast_ref::<gtk::FlowBoxChild>() {
                    if let Some(button) = child_row
                        .child()
                        .and_then(|c| c.downcast::<gtk::Button>().ok())
                    {
                        unsafe {
                            if let Some(name) =
                                button.data::<String>("icon-name").map(|p| p.as_ref())
                            {
                                child_row.set_visible(name.to_lowercase().contains(&text));
                            }
                        }
                    }
                }
                child = widget.next_sibling();
            }
        });

        let dialog_select = dialog.clone();
        flow_box.connect_child_activated(move |_, _| {
            dialog_select.response(gtk::ResponseType::Ok);
        });
        dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
        dialog.add_button(&crate::i18n::tr("Select"), gtk::ResponseType::Ok);

        let flow_box_select = flow_box.clone();
        let sender_clone = sender.clone();
        let target_path_clone = target_path.clone();

        dialog.connect_response(move |win, response| {
            if response == gtk::ResponseType::Ok {
                if let Some(row) = flow_box_select.selected_children().first() {
                    if let Some(button) = row.child().and_then(|c| c.downcast::<gtk::Button>().ok())
                    {
                        unsafe {
                            if let Some(name) =
                                button.data::<String>("icon-name").map(|p| p.as_ref())
                            {
                                sender_clone.input(AppMsg::SetFolderIcon {
                                    path: target_path_clone.clone(),
                                    icon_name: name.to_string(),
                                });
                            }
                        }
                    }
                }
            }
            win.destroy();
        });
        dialog.present();
    }

    /// Displays a grid-based icon picker dialog to set custom folder icons.
    pub fn show_icon_picker(&self, target_path: PathBuf, sender: &AsyncComponentSender<Self>) {
        let toplevels = gtk::Window::list_toplevels();
        let parent = toplevels
            .first()
            .and_then(|w| w.downcast_ref::<gtk::Window>());
        let dialog = gtk::Dialog::builder()
            .title("Select Folder Icon")
            .transient_for(parent.unwrap())
            .modal(true)
            .use_header_bar(1)
            .build();
        let flow_box = gtk::FlowBox::builder()
            .valign(gtk::Align::Start)
            .max_children_per_line(6)
            .min_children_per_line(6)
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&flow_box)
            .height_request(350)
            .width_request(400)
            .build();
        let search_entry = gtk::SearchEntry::builder()
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        let content_area = dialog.content_area();
        content_area.append(&search_entry);
        content_area.append(&scrolled);
        let icon_theme = gtk::IconTheme::for_display(&gdk::Display::default().unwrap());
        let icon_names = icon_theme.icon_names();
        for icon_name in icon_names {
            let name_str = icon_name.as_str();
            if name_str.contains("folder") || name_str.contains("Folder") {
                let image = gtk::Image::from_icon_name(name_str);
                image.set_icon_size(gtk::IconSize::Large);
                let button = gtk::Button::builder()
                    .child(&image)
                    .tooltip_text(name_str)
                    .has_frame(false)
                    .build();
                unsafe {
                    button.set_data("icon-name", icon_name.to_string());
                }
                let dialog_btn_clone = dialog.clone();
                let flow_box_btn_clone = flow_box.clone();
                button.connect_clicked(move |btn| {
                    if let Some(row) = btn
                        .parent()
                        .and_then(|p| p.downcast::<gtk::FlowBoxChild>().ok())
                    {
                        flow_box_btn_clone.select_child(&row);
                        dialog_btn_clone.response(gtk::ResponseType::Ok);
                    }
                });
                flow_box.append(&button);
            }
        }
        let flow_box_clone = flow_box.clone();
        search_entry.connect_search_changed(move |entry| {
            let text = entry.text().to_string().to_lowercase();
            let mut child = flow_box_clone.first_child();
            while let Some(ref widget) = child {
                if let Some(child_row) = widget.downcast_ref::<gtk::FlowBoxChild>() {
                    if let Some(button) = child_row
                        .child()
                        .and_then(|c| c.downcast::<gtk::Button>().ok())
                    {
                        unsafe {
                            if let Some(name) =
                                button.data::<String>("icon-name").map(|p| p.as_ref())
                            {
                                child_row.set_visible(name.to_lowercase().contains(&text));
                            }
                        }
                    }
                }
                child = widget.next_sibling();
            }
        });
        let dialog_select = dialog.clone();
        flow_box.connect_child_activated(move |_, _| {
            dialog_select.response(gtk::ResponseType::Ok);
        });
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        dialog.add_button("Select", gtk::ResponseType::Ok);

        let flow_box_select = flow_box.clone();
        let sender_clone = sender.clone();
        let target_path_clone = target_path.clone();

        dialog.connect_response(move |win, response| {
            if response == gtk::ResponseType::Ok {
                if let Some(row) = flow_box_select.selected_children().first() {
                    if let Some(button) = row.child().and_then(|c| c.downcast::<gtk::Button>().ok())
                    {
                        unsafe {
                            if let Some(name) =
                                button.data::<String>("icon-name").map(|p| p.as_ref())
                            {
                                sender_clone.input(AppMsg::SetFolderIcon {
                                    path: target_path_clone.clone(),
                                    icon_name: name.to_string(),
                                });
                            }
                        }
                    }
                }
            }
            win.destroy();
        });
        dialog.present();
    }

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
