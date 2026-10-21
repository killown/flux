use crate::model::{AppMsg, FluxApp};
use adw::gdk;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;
use std::rc::Rc;

impl FluxApp {
    /// Displays a modal prompt to create a new sidebar section header.
    pub fn show_prompt_new_sidebar_section(&self, sender: &relm4::AsyncComponentSender<Self>) {
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();

        let dialog = adw::AlertDialog::new(
            Some(&crate::i18n::tr("New Section")),
            Some(&crate::i18n::tr("Enter a title for the new section:")),
        );

        dialog.add_response("cancel", &crate::i18n::tr("Cancel"));
        dialog.add_response("ok", &crate::i18n::tr("Create"));
        dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("cancel");

        let entry = gtk::Entry::builder()
            .placeholder_text(crate::i18n::tr("Section title"))
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
                let title = entry_clone.text().trim().to_string();
                s.input(AppMsg::AddSidebarSection(title));
            }
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

        let dialog = adw::AlertDialog::new(
            Some(&crate::i18n::tr("Rename Section")),
            Some(&crate::i18n::tr("Enter a new title for this section:")),
        );

        dialog.add_response("cancel", &crate::i18n::tr("Cancel"));
        dialog.add_response("ok", &crate::i18n::tr("Rename"));
        dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("cancel");

        let entry = gtk::Entry::builder()
            .text(&current_name)
            .activates_default(true)
            .build();
        entry.select_region(0, -1);
        entry.connect_map(|e| {
            e.grab_focus();
        });
        dialog.set_extra_child(Some(&entry));
        dialog.present(parent.as_ref());

        let entry_clone = entry.clone();
        dialog.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let new_name = entry_clone.text().trim().to_string();
                if new_name != old_name {
                    s.input(AppMsg::RenameSidebarSection {
                        old_name: old_name.clone(),
                        new_name,
                    });
                }
            }
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

        let dialog = adw::AlertDialog::new(
            Some(&crate::i18n::tr("Rename Bookmark")),
            Some(&crate::i18n::tr(
                "Enter a new display name for this bookmark:",
            )),
        );

        dialog.add_response("cancel", &crate::i18n::tr("Cancel"));
        dialog.add_response("ok", &crate::i18n::tr("Rename"));
        dialog.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("cancel");

        let entry = gtk::Entry::builder()
            .text(&current_name)
            .activates_default(true)
            .build();

        entry.select_region(0, -1);
        entry.connect_map(|e| {
            e.grab_focus();
        });

        dialog.set_extra_child(Some(&entry));
        dialog.present(parent.as_ref());

        let entry_clone = entry.clone();
        dialog.connect_response(None, move |_, resp| {
            if resp == "ok" {
                let new_name = entry_clone.text().trim().to_string();
                if !new_name.is_empty() {
                    s.input(AppMsg::RenameSidebarPlace {
                        path: target_path.clone(),
                        new_name,
                    });
                }
            }
        });
    }

    /// Displays a symbolic-only icon picker dialog for sidebar locations.
    pub fn show_sidebar_icon_picker(
        &self,
        target_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("show_sidebar_icon_picker");
        let toplevels = gtk::Window::list_toplevels();
        let parent = toplevels
            .first()
            .and_then(|w| w.downcast_ref::<gtk::Window>());
        let dialog = adw::Dialog::builder()
            .title(crate::i18n::tr("Select Sidebar Icon"))
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
        let content_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content_area.append(&search_entry);
        content_area.append(&scrolled);

        let header = adw::HeaderBar::new();
        let cancel_btn = gtk::Button::with_label(&crate::i18n::tr("Cancel"));
        let select_btn = gtk::Button::with_label(&crate::i18n::tr("Select"));
        select_btn.add_css_class("suggested-action");
        header.pack_start(&cancel_btn);
        header.pack_end(&select_btn);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content_area));
        dialog.set_child(Some(&toolbar_view));

        let confirm: Rc<dyn Fn()> = Rc::new({
            let flow_box_select = flow_box.clone();
            let sender_clone = sender.clone();
            let target_path_clone = target_path.clone();
            let dialog_weak = dialog.downgrade();
            move || {
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
                if let Some(dlg) = dialog_weak.upgrade() {
                    dlg.close();
                }
            }
        });

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
                let confirm_btn_clone = confirm.clone();
                let flow_box_btn_clone = flow_box.clone();
                button.connect_clicked(move |btn| {
                    if let Some(row) = btn
                        .parent()
                        .and_then(|p| p.downcast::<gtk::FlowBoxChild>().ok())
                    {
                        flow_box_btn_clone.select_child(&row);
                        confirm_btn_clone();
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

        let confirm_activate = confirm.clone();
        flow_box.connect_child_activated(move |_, _| confirm_activate());

        let confirm_select = confirm.clone();
        select_btn.connect_clicked(move |_| confirm_select());

        let dialog_cancel = dialog.downgrade();
        cancel_btn.connect_clicked(move |_| {
            if let Some(dlg) = dialog_cancel.upgrade() {
                dlg.close();
            }
        });

        dialog.present(parent);
    }
}
