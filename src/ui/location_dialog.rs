use crate::model::AppMsg;
use crate::model::FluxApp;
use adw::prelude::*;
use gtk::gdk;
use gtk::glib;
use relm4::prelude::*;
use relm4::RelmRemoveAllExt;
use std::path::PathBuf;

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
