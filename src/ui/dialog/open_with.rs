use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn show_open_with_dialog(&self, target_file: PathBuf, sender: &AsyncComponentSender<Self>) {
        let window = gtk::Application::default().active_window();
        let s = sender.clone();

        let dialog = adw::Window::builder()
            .title(crate::i18n::tr("Open With…"))
            .modal(true)
            .default_width(460)
            .default_height(520)
            .resizable(false)
            .build();

        if let Some(ref win) = window {
            dialog.set_transient_for(Some(win));
        }

        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();

        // ── Header Bar ────────────────────────────────────────────────────────
        let header = adw::HeaderBar::builder()
            .show_end_title_buttons(false)
            .show_start_title_buttons(false)
            .build();

        let title_widget = adw::WindowTitle::builder()
            .title(crate::i18n::tr("Open With"))
            .build();
        header.set_title_widget(Some(&title_widget));

        let cancel_btn = gtk::Button::builder()
            .label(crate::i18n::tr("Cancel"))
            .build();
        header.pack_start(&cancel_btn);

        let open_btn = gtk::Button::builder()
            .label(crate::i18n::tr("Open"))
            .css_classes(["suggested-action"])
            .build();
        header.pack_end(&open_btn);

        root.append(&header);

        // ── Content Area ──────────────────────────────────────────────────────
        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .margin_start(16)
            .margin_end(16)
            .margin_top(12)
            .margin_bottom(12)
            .vexpand(true)
            .build();

        // Header info displaying file name and detected MIME type
        let file_name = target_file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| target_file.display().to_string());

        let (mime_type, _) = gtk::gio::content_type_guess(Some(&file_name), None);
        let label_text = format!(
            "<b>{}</b>\n<span size=\"smaller\" alpha=\"70%\">{}</span>",
            glib::markup_escape_text(file_name.as_str()),
            glib::markup_escape_text(mime_type.as_str())
        );

        let mime_label = gtk::Label::builder()
            .label(label_text.as_str())
            .use_markup(true)
            .xalign(0.0)
            .build();
        content_box.append(&mime_label);

        // Live Search Entry
        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text(crate::i18n::tr("Search applications…"))
            .build();
        content_box.append(&search_entry);

        // Scrolled ListBox for Apps
        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .child(&list_box)
            .build();
        content_box.append(&scrolled);

        // Query the single true system default application
        let default_app = gtk::gio::AppInfo::default_for_type(&mime_type, false);

        // Gather recommended and all installed desktop applications
        let recommended = gtk::gio::AppInfo::recommended_for_type(&mime_type);
        let all_apps = gtk::gio::AppInfo::all();

        let mut seen_ids = std::collections::HashSet::new();

        // Helper to construct a row
        let make_row = |app: &gtk::gio::AppInfo, is_default: bool| -> gtk::ListBoxRow {
            let row = gtk::ListBoxRow::new();
            let hbox = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .margin_start(10)
                .margin_end(10)
                .margin_top(8)
                .margin_bottom(8)
                .build();

            let icon_widget = if let Some(gicon) = app.icon() {
                gtk::Image::from_gicon(&gicon)
            } else {
                gtk::Image::from_icon_name("application-x-executable")
            };
            icon_widget.set_pixel_size(32);
            hbox.append(&icon_widget);

            let text_vbox = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(2)
                .hexpand(true)
                .build();

            let name_label = gtk::Label::builder()
                .label(app.display_name().as_str())
                .xalign(0.0)
                .css_classes(["heading"])
                .build();
            text_vbox.append(&name_label);

            if let Some(desc) = app.description() {
                let desc_label = gtk::Label::builder()
                    .label(desc.as_str())
                    .xalign(0.0)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .css_classes(["caption", "dim-label"])
                    .build();
                text_vbox.append(&desc_label);
            }

            hbox.append(&text_vbox);

            if is_default {
                let badge = gtk::Label::builder()
                    .label(crate::i18n::tr("Default"))
                    .css_classes(["tag", "caption"])
                    .valign(gtk::Align::Center)
                    .build();
                hbox.append(&badge);
            }

            row.set_child(Some(&hbox));

            // Attach search text metadata
            let searchable = format!(
                "{} {} {}",
                app.display_name(),
                app.executable().to_string_lossy(),
                app.description().unwrap_or_default()
            )
            .to_lowercase();

            unsafe {
                row.set_data("app-info", app.clone());
                row.set_data("search-meta", searchable);
            }

            row
        };

        // Insert the default application first if available
        if let Some(ref def_app) = default_app {
            if let Some(id) = def_app.id() {
                seen_ids.insert(id.to_string());
            }
            list_box.append(&make_row(def_app, true));
        }

        // Populate remaining recommended apps
        for app in recommended {
            let app_id_str = app.id().map(|id| id.to_string());
            if let Some(ref id) = app_id_str {
                if seen_ids.contains(id.as_str()) {
                    continue;
                }
                seen_ids.insert(id.clone());
            }
            list_box.append(&make_row(&app, false));
        }

        // Populate remaining installed applications
        for app in all_apps {
            if !app.should_show() {
                continue;
            }
            if let Some(id) = app.id() {
                if seen_ids.contains(id.as_str()) {
                    continue;
                }
                seen_ids.insert(id.to_string());
            }
            list_box.append(&make_row(&app, false));
        }

        // Select the first item (default app) automatically
        if let Some(first_row) = list_box.row_at_index(0) {
            list_box.select_row(Some(&first_row));
        }

        // Search filtering logic
        let list_box_filter = list_box.clone();
        search_entry.connect_search_changed(move |entry| {
            let query = entry.text().to_lowercase();
            let mut child = list_box_filter.first_child();
            while let Some(w) = child {
                if let Some(row) = w.downcast_ref::<gtk::ListBoxRow>() {
                    unsafe {
                        if let Some(meta) = row.data::<String>("search-meta").map(|p| p.as_ref()) {
                            row.set_visible(query.is_empty() || meta.contains(&query));
                        }
                    }
                }
                child = w.next_sibling();
            }
        });

        // "Always use for this file type" CheckButton
        let set_default_check = gtk::CheckButton::builder()
            .label(crate::i18n::tr("Always use for this file type"))
            .active(false)
            .margin_top(8)
            .margin_bottom(4)
            .margin_start(4)
            .build();
        content_box.append(&set_default_check);

        root.append(&content_box);
        dialog.set_content(Some(&root));

        // ── Execution Closure ─────────────────────────────────────────────────
        let target_clone = target_file.clone();
        let mime_type_str = mime_type.to_string();
        let list_box_exec = list_box.clone();
        let set_default_exec = set_default_check.clone();
        let s_exec = s.clone();
        let d_exec = dialog.clone();

        let execute_launch = move || {
            if let Some(row) = list_box_exec.selected_row() {
                unsafe {
                    if let Some(app) = row
                        .data::<gtk::gio::AppInfo>("app-info")
                        .map(|p| p.as_ref())
                    {
                        let gfile = gtk::gio::File::for_path(&target_clone);

                        if set_default_exec.is_active() {
                            if let Err(e) = app.set_as_default_for_type(&mime_type_str) {
                                s_exec.input(AppMsg::ShowToast(format!(
                                    "Failed to set default: {e}"
                                )));
                            }
                        }

                        if let Err(e) = app.launch(&[gfile], gtk::gio::AppLaunchContext::NONE) {
                            s_exec.input(AppMsg::ShowToast(format!("Failed to launch: {e}")));
                        }
                    }
                }
            }
            d_exec.close();
        };

        // Open button click
        let exec_on_btn = execute_launch.clone();
        open_btn.connect_clicked(move |_| {
            exec_on_btn();
        });

        // Double-click or Enter on row
        let exec_on_row = execute_launch;
        list_box.connect_row_activated(move |_, _| {
            exec_on_row();
        });

        // Cancel button click
        let d_cancel = dialog.clone();
        cancel_btn.connect_clicked(move |_| {
            d_cancel.close();
        });

        // Escape key closes
        let d_esc = dialog.clone();
        let key_ctrl = gtk::EventControllerKey::new();
        key_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == adw::gdk::Key::Escape {
                d_esc.close();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl);

        // Autofocus search entry on map
        let search_focus = search_entry.clone();
        dialog.connect_map(move |_| {
            search_focus.grab_focus();
        });

        dialog.present();
    }
}
