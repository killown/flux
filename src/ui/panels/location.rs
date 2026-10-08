use super::header::panel_header;
use super::resize::resizable_panel;
use super::spec::PanelSpec;
use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::services::db::StateManager;
use adw::prelude::*;
use relm4::{AsyncComponentSender, RelmRemoveAllExt};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

/// Builds one history row with a delete button.
fn history_row(uri: &str, db: &Arc<StateManager>, list: &gtk::ListBox) -> gtk::ListBoxRow {
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
        .css_classes(["flat"])
        .build();

    let label = gtk::Label::builder()
        .label(uri)
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .build();

    row_box.append(&delete_btn);
    row_box.append(&label);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&row_box));

    let uri = uri.to_string();
    let db = db.clone();
    let list = list.clone();
    let row_weak = row.downgrade();
    delete_btn.connect_clicked(move |_| {
        let _ = db.remove_location(&uri);
        if let Some(row) = row_weak.upgrade() {
            list.remove(&row);
        }
    });

    row
}

/// Builds the location panel and returns it together with its path entry.
pub fn build_location_panel(
    current_path: &str,
    state_db: Arc<StateManager>,
    sender: AsyncComponentSender<FluxApp>,
) -> (gtk::Box, gtk::Entry) {
    let spec = PanelSpec::location();

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(spec.effective())
        .hexpand(false)
        .build();
    panel.add_css_class("sidebar");

    let entry = gtk::Entry::builder()
        .text(current_path)
        .hexpand(true)
        .build();

    let go_btn = gtk::Button::builder()
        .label(tr("Connect"))
        .css_classes(["suggested-action", "pill"])
        .valign(gtk::Align::Center)
        .build();

    {
        let s = sender.clone();
        let header = panel_header(
            Some("folder-open-symbolic"),
            &tr("Enter Location"),
            &[go_btn.clone().upcast::<gtk::Widget>()],
            move || s.input(AppMsg::ToggleLocationPanel),
        );
        panel.append(&header);
    }

    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .margin_top(4)
        .margin_bottom(12)
        .vexpand(true)
        .build();

    let hint = gtk::Label::builder()
        .label(tr(
            "Type a local path or network URI (e.g., smb://server/share, sftp://host, /home):",
        ))
        .css_classes(["caption", "dim-label"])
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .wrap(true)
        .build();

    let history_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(["boxed-list"])
        .build();

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(false)
        .hexpand(false)
        .vexpand(true)
        .child(&history_list)
        .build();

    let clear_btn = gtk::Button::builder()
        .label(tr("Clear History"))
        .halign(gtk::Align::End)
        .build();

    content_box.append(&hint);
    content_box.append(&entry);
    content_box.append(&scroll);
    content_box.append(&clear_btn);
    panel.append(&content_box);

    // Rebuilds the history list, keeping only URIs that contain `filter`.
    let populate: Rc<dyn Fn(&str)> = {
        let list = history_list.clone();
        let scroll = scroll.clone();
        let clear_btn = clear_btn.clone();
        let db = state_db.clone();
        Rc::new(move |filter: &str| {
            list.remove_all();
            let filter_lc = filter.to_lowercase();
            let history = db.get_location_history().unwrap_or_default();
            let mut count = 0;
            for uri in history {
                if filter_lc.is_empty() || uri.to_lowercase().contains(&filter_lc) {
                    list.append(&history_row(&uri, &db, &list));
                    count += 1;
                    if count >= 100 {
                        break;
                    }
                }
            }
            scroll.set_visible(count > 0);
            clear_btn.set_visible(count > 0);
        })
    };

    // Saves the typed location to history and navigates to it.
    let submit: Rc<dyn Fn()> = {
        let entry = entry.clone();
        let db = state_db.clone();
        let s = sender.clone();
        Rc::new(move || {
            let text = entry.text();
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return;
            }
            let _ = db.add_location(trimmed);

            let target = if crate::services::network::is_network_uri(Path::new(trimmed))
                || trimmed.starts_with(crate::services::archive::ARCHIVE_URI)
                || trimmed.starts_with("trash:///")
                || trimmed.starts_with("recent:///")
            {
                PathBuf::from(trimmed)
            } else {
                FluxApp::expand_path(trimmed)
            };

            s.input(AppMsg::ToggleLocationPanel);
            s.input(AppMsg::Navigate(target));
        })
    };

    {
        let submit = submit.clone();
        entry.connect_activate(move |_| submit());
    }
    {
        let submit = submit.clone();
        go_btn.connect_clicked(move |_| submit());
    }

    {
        let populate = populate.clone();
        entry.connect_changed(move |e| populate(&e.text()));
    }

    {
        let db = state_db.clone();
        let populate = populate.clone();
        clear_btn.connect_clicked(move |_| {
            let _ = db.clear_location_history();
            populate("");
        });
    }

    // Clicking a history row fills the entry so it can be edited before connecting.
    {
        let entry = entry.clone();
        history_list.connect_row_activated(move |_, row| {
            let label = row
                .child()
                .and_downcast::<gtk::Box>()
                .and_then(|b| b.last_child())
                .and_downcast::<gtk::Label>();
            if let Some(label) = label {
                entry.set_text(&label.text());
                entry.grab_focus();
                entry.set_position(-1);
            }
        });
    }

    // Down arrow in the entry jumps to the history list.
    let down_ctrl = gtk::EventControllerKey::new();
    {
        let list = history_list.clone();
        down_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == adw::gdk::Key::Down {
                if let Some(row) = list.row_at_index(0) {
                    list.select_row(Some(&row));
                    row.grab_focus();
                    return gtk::glib::Propagation::Stop;
                }
            }
            gtk::glib::Propagation::Proceed
        });
    }
    entry.add_controller(down_ctrl);

    // Each time the panel is shown: reset the history filter and focus the entry.
    {
        let entry = entry.clone();
        let populate = populate.clone();
        panel.connect_map(move |_| {
            populate("");
            entry.grab_focus();
            entry.select_region(0, -1);
        });
    }

    let root = resizable_panel(&spec, &panel, |_| {});

    let esc_ctrl = gtk::EventControllerKey::new();
    esc_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let s = sender.clone();
        esc_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == adw::gdk::Key::Escape {
                s.input(AppMsg::ToggleLocationPanel);
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
    }
    root.add_controller(esc_ctrl);

    (root, entry)
}
