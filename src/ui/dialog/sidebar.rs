use crate::model::{AppMsg, FluxApp};
use adw::gdk;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
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
                                    if Self::expand_path(&place.path) == target_path_clone {
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
}
