use crate::model::{AppMsg, FluxApp};
use adw::gdk;
use adw::prelude::*;
use relm4::prelude::*;
use relm4::RelmRemoveAllExt;
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
                            s.input(AppMsg::InvalidateCacheAndNavigate(folder_path));
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

    /// Displays a modal prompt to create a new sidebar section header.
    pub fn show_prompt_new_sidebar_section(&self, sender: &relm4::AsyncComponentSender<Self>) {
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let dialog = gtk::MessageDialog::new(
            parent.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Other,
            gtk::ButtonsType::None,
            crate::i18n::tr("New Section"),
        );
        dialog.set_secondary_text(Some(&crate::i18n::tr("Enter a title for the new section:")));

        dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
        let ok_btn = dialog.add_button(&crate::i18n::tr("Create"), gtk::ResponseType::Ok);
        ok_btn.style_context().add_class("suggested-action");
        dialog.set_default_response(gtk::ResponseType::Ok);

        let entry = gtk::Entry::builder()
            .placeholder_text(crate::i18n::tr("Section title"))
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
                let title = entry_clone.text().trim().to_string();
                s.input(AppMsg::AddSidebarSection(title));
            }
            dlg.close();
        });
    }

    /// Displays a modal prompt to rename an existing sidebar section header.
    pub fn show_prompt_sidebar_rename_section(
        &self,
        old_name: String,
        current_name: String,
        sender: &relm4::AsyncComponentSender<Self>,
    ) {
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let dialog = gtk::MessageDialog::new(
            parent.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Other,
            gtk::ButtonsType::None,
            "",
        );

        let title_markup = format!(
            "<span weight=\"bold\" size=\"medium\">{}</span>",
            crate::i18n::tr("Rename Section")
        );
        dialog.set_markup(&title_markup);
        dialog.set_secondary_text(Some(&crate::i18n::tr(
            "Enter a new title for this section:",
        )));

        let message_area = dialog.message_area();
        message_area.set_margin_top(12);
        message_area.set_margin_bottom(6);
        message_area.set_margin_start(12);
        message_area.set_margin_end(12);

        // Prevent GtkMessageDialog's primary title label from line-wrapping
        let mut child = message_area.first_child();
        while let Some(w) = child {
            if let Some(label) = w.downcast_ref::<gtk::Label>() {
                label.set_wrap(false);
                label.set_ellipsize(gtk::pango::EllipsizeMode::None);
                break;
            }
            child = w.next_sibling();
        }

        dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
        let ok_btn = dialog.add_button(&crate::i18n::tr("Rename"), gtk::ResponseType::Ok);
        ok_btn.style_context().add_class("suggested-action");
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
                if new_name != old_name {
                    s.input(AppMsg::RenameSidebarSection {
                        old_name: old_name.clone(),
                        new_name,
                    });
                }
            }
            dlg.close();
        });
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
                                let mut config = crate::utils::load_config();
                                let path_str = target_path_clone.to_string_lossy().to_string();
                                let path_trimmed = path_str.trim_end_matches('/').to_string();

                                let mut matched = false;
                                for place in &mut config.sidebar {
                                    if crate::utils::expand_path(&place.path) == target_path_clone {
                                        place.icon = name.to_string();
                                        matched = true;
                                    }
                                }

                                if !matched {
                                    if let Some(device) =
                                        config.ui.device_renames.get_mut(&path_str)
                                    {
                                        device.icon = Some(name.to_string());
                                    } else if let Some(device) =
                                        config.ui.device_renames.get_mut(&path_trimmed)
                                    {
                                        device.icon = Some(name.to_string());
                                    } else {
                                        let display_name = target_path_clone
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| path_trimmed);

                                        config.ui.device_renames.insert(
                                            path_str,
                                            crate::model::DeviceRename {
                                                name: display_name,
                                                icon: Some(name.to_string()),
                                            },
                                        );
                                    }
                                }

                                crate::utils::save_config(&config);
                                sender_clone.input(AppMsg::RefreshSidebar);
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

    /// Presents a warning dialog when pasting into a location where target folders already exist.
    pub fn show_confirm_replace_paste(
        &self,
        files: Vec<gio::File>,
        conflicts: Vec<String>,
        is_cut: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        let window = gtk::Application::default().active_window();
        let body = if conflicts.len() == 1 {
            format!(
                "\"{}\" already exists in this location. Replace it and merge its contents?",
                conflicts[0]
            )
        } else {
            format!(
                "{} folders already exist in this location. Replace them and merge their contents?",
                conflicts.len()
            )
        };
        let dialog = gtk::MessageDialog::new(
            window.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Warning,
            gtk::ButtonsType::None,
            "Replace Existing Folder?",
        );
        dialog.set_secondary_text(Some(&body));
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);

        let replace_btn = dialog.add_button("Replace", gtk::ResponseType::Accept);
        replace_btn.style_context().add_class("destructive-action");

        let s = sender.clone();
        dialog.connect_response(move |dlg, response| {
            dlg.close();
            if response == gtk::ResponseType::Accept {
                s.input(AppMsg::PerformPasteForced {
                    files: files.clone(),
                    is_cut,
                });
            }
        });
        dialog.present();
    }

    pub fn show_custom_icon_file_chooser(
        target_path: PathBuf,
        toast: Option<String>,
        sender: AsyncComponentSender<Self>,
    ) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Images"));
        filter.add_mime_type("image/png");
        filter.add_mime_type("image/jpeg");
        filter.add_mime_type("image/webp");
        filter.add_mime_type("image/svg+xml");

        let toplevels = gtk::Window::list_toplevels();
        let parent = toplevels
            .first()
            .and_then(|w| w.downcast_ref::<gtk::Window>())
            .cloned();

        let chooser = gtk::FileChooserNative::builder()
            .title(crate::i18n::tr("Select Custom Icon Image"))
            .action(gtk::FileChooserAction::Open)
            .accept_label(crate::i18n::tr("Set Icon"))
            .cancel_label(crate::i18n::tr("Cancel"))
            .build();

        if let Some(ref win) = parent {
            chooser.set_transient_for(Some(win));
        }
        chooser.add_filter(&filter);

        let chooser_ref = chooser.clone();
        chooser.connect_response(move |_, response| {
            if response == gtk::ResponseType::Accept {
                if let Some(file) = chooser_ref.file() {
                    if let Some(image_path) = file.path() {
                        sender.input(AppMsg::SetFileIcon {
                            path: target_path.clone(),
                            image_path,
                        });
                        if let Some(ref msg) = toast {
                            sender.input(AppMsg::ShowToast(msg.clone()));
                        }
                    }
                }
            }
        });
        chooser.show();
    }

    pub fn show_location_dialog(app: &mut FluxApp, sender: AsyncComponentSender<FluxApp>) {
        let window = gtk::Application::default().active_window();
        let s = sender.clone();
        let state_db = app.state_db.clone();
        let current_path_str = app.current_path.to_string_lossy().to_string();

        let dialog = gtk::MessageDialog::new(
            window.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Other,
            gtk::ButtonsType::None,
            crate::i18n::tr("Enter Location"),
        );

        dialog.set_secondary_text(Some(&crate::i18n::tr(
            "Type a local path or network URI (e.g., smb://server/share, sftp://host, /home):",
        )));

        dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
        let go_btn = dialog.add_button(&crate::i18n::tr("Connect"), gtk::ResponseType::Ok);
        go_btn.style_context().add_class("suggested-action");
        dialog.set_default_response(gtk::ResponseType::Ok);

        let content_area = dialog.content_area();

        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(16)
            .margin_end(16)
            .build();

        let entry = gtk::Entry::builder()
            .text(&current_path_str)
            .activates_default(true)
            .build();

        entry.select_region(0, -1);
        entry.connect_map(|e| {
            e.grab_focus();
        });

        // Suggestion list box for history autocomplete
        let history_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .visible(false)
            .build();

        let scrolled_history = gtk::ScrolledWindow::builder()
            .child(&history_list)
            .max_content_height(150)
            .propagate_natural_height(true)
            .visible(false)
            .build();

        // Clear history button
        let clear_history_btn = gtk::Button::builder()
            .label(crate::i18n::tr("Clear History"))
            .halign(gtk::Align::End)
            .build();

        let db_for_clear = state_db.clone();
        let history_list_clone = history_list.clone();
        let scrolled_clone = scrolled_history.clone();
        clear_history_btn.connect_clicked(move |_| {
            let _ = db_for_clear.clear_location_history();
            history_list_clone.remove_all();
            scrolled_clone.set_visible(false);
        });

        // Helper closure to query and populate history suggestions
        let db_for_populate = state_db.clone();
        let populate_history = {
            let history_list_p = history_list.clone();
            let scrolled_p = scrolled_history.clone();
            let db_for_delete = state_db.clone();

            move |filter: &str| {
                history_list_p.remove_all();
                if let Ok(history) = db_for_populate.get_location_history() {
                    let filter_lc = filter.to_lowercase();
                    let mut count = 0;
                    for uri in history {
                        if filter.is_empty() || uri.to_lowercase().contains(&filter_lc) {
                            // Row container box
                            let row_box = gtk::Box::builder()
                                .orientation(gtk::Orientation::Horizontal)
                                .spacing(6)
                                .margin_start(4)
                                .margin_end(8)
                                .margin_top(4)
                                .margin_bottom(4)
                                .build();

                            let delete_btn = gtk::Button::builder()
                                .icon_name("window-close-symbolic")
                                .valign(gtk::Align::Center)
                                .css_classes(vec!["flat".to_string()])
                                .build();

                            let row_label = gtk::Label::builder()
                                .label(&uri)
                                .xalign(0.0)
                                .hexpand(true)
                                .ellipsize(pango::EllipsizeMode::Middle)
                                .build();

                            let uri_to_delete = uri.clone();
                            let db_del = db_for_delete.clone();
                            let list_ref = history_list_p.clone();
                            let row_box_ref = row_box.clone();

                            delete_btn.connect_clicked(move |_| {
                                let _ = db_del.remove_location(&uri_to_delete);
                                if let Some(parent) = row_box_ref.parent() {
                                    list_ref.remove(&parent);
                                }
                            });

                            row_box.append(&delete_btn);
                            row_box.append(&row_label);

                            // Wrap each row inside a ListBoxRow so GTK can select and activate it properly!
                            let row = gtk::ListBoxRow::new();
                            row.set_child(Some(&row_box));
                            history_list_p.append(&row);

                            count += 1;
                            if count >= 100 {
                                break;
                            }
                        }
                    }
                    let has_items = count > 0;
                    history_list_p.set_visible(has_items);
                    scrolled_p.set_visible(has_items);
                }
            }
        };
        let populate_clone = populate_history.clone();
        entry.connect_changed(move |e| {
            populate_clone(&e.text());
        });

        // Trigger suggestion dropdown on Down arrow key press
        let key_controller = gtk::EventControllerKey::new();
        let populate_key = populate_history.clone();
        let entry_key = entry.clone();
        let scrolled_key = scrolled_history.clone();

        key_controller.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gdk::Key::Down {
                populate_key(&entry_key.text());
                return glib::Propagation::Stop;
            } else if keyval == gdk::Key::Escape {
                scrolled_key.set_visible(false);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        entry.add_controller(key_controller);

        // Populate entry when clicking a history suggestion row
        let entry_select = entry.clone();
        let scrolled_select = scrolled_history.clone();
        history_list.connect_row_activated(move |_, row| {
            if let Some(row_box) = row.child().and_downcast::<gtk::Box>() {
                if let Some(label) = row_box.last_child().and_downcast::<gtk::Label>() {
                    entry_select.set_text(&label.text());
                    scrolled_select.set_visible(false);
                    entry_select.grab_focus();
                }
            }
        });

        vbox.append(&entry);
        vbox.append(&scrolled_history);
        vbox.append(&clear_history_btn);
        content_area.append(&vbox);
        dialog.present();

        let entry_clone = entry.clone();
        let db_submit = state_db.clone();

        dialog.connect_response(move |dlg, resp| {
            if resp == gtk::ResponseType::Ok {
                let text = entry_clone.text().to_string();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let _ = db_submit.add_location(trimmed);

                    if crate::services::network::is_network_uri(std::path::Path::new(trimmed))
                        || trimmed.starts_with(crate::services::archive::ARCHIVE_URI)
                        || trimmed.starts_with("trash:///")
                        || trimmed.starts_with("recent:///")
                    {
                        s.input(AppMsg::Navigate(PathBuf::from(trimmed)));
                    } else {
                        let expanded = crate::utils::expand_path(trimmed);
                        s.input(AppMsg::Navigate(expanded));
                    }
                }
            }
            dlg.close();
        });
    }

    pub fn show_about_window() {
        let about = gtk::AboutDialog::builder()
            .program_name("Flux")
            .version(env!("CARGO_PKG_VERSION"))
            .logo_icon_name("system-file-manager")
            .authors(vec!["killown".to_string()])
            .website("https://github.com/killown/flux")
            .website_label(crate::i18n::tr("Source Code"))
            .comments(crate::i18n::tr(
                "A fast, keyboard-driven file manager built with GTK4 and Libadwaita.",
            ))
            .license_type(gtk::License::Gpl30Only)
            .modal(true)
            .resizable(false)
            .build();

        if let Some(window) = gtk::Application::default().active_window() {
            about.set_transient_for(Some(&window));
        }

        about.present();
    }
}
