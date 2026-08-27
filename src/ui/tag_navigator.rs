use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;

/// Constructs and presents a command-palette style tag navigator dialog.
pub fn show_tag_navigator(
    parent: &impl IsA<gtk::Widget>,
    available_tags: Vec<String>,
    sender: AsyncComponentSender<FluxApp>,
) {
    let parent_window = parent.root().and_downcast::<gtk::Window>();

    let dialog = adw::Window::builder()
        .title(tr("Browse by Tag").as_str())
        .modal(true)
        .default_width(440)
        .default_height(480)
        .resizable(false)
        .build();

    if let Some(ref win) = parent_window {
        dialog.set_transient_for(Some(win));
    }

    let root_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    // ── Header Bar ───────────────────────────────────────────────────────────
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(true);
    header.set_show_start_title_buttons(false);

    let title_label = gtk::Label::builder()
        .label(tr("Tag Navigator").as_str())
        .css_classes(["title-4", "heading"])
        .build();
    header.set_title_widget(Some(&title_label));
    root_box.append(&header);

    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .vexpand(true)
        .build();

    // ── Live Search Entry ────────────────────────────────────────────────────
    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(tr("Type tag name…").as_str())
        .hexpand(true)
        .build();
    content_box.append(&search_entry);

    // ── Tag Results List ─────────────────────────────────────────────────────
    let list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(["boxed-list"])
        .build();

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&list_box)
        .build();

    let empty_status = adw::StatusPage::builder()
        .icon_name("tag-symbolic")
        .title(tr("No Tags Found").as_str())
        .description(tr("No matching tags in database").as_str())
        .vexpand(true)
        .build();

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vexpand(true)
        .build();

    stack.add_named(&scroll, Some("list"));
    stack.add_named(&empty_status, Some("empty"));
    content_box.append(&stack);

    root_box.append(&content_box);
    dialog.set_content(Some(&root_box));

    let all_tags = Rc::new(
        available_tags
            .into_iter()
            .map(|t| t.trim_start_matches('#').to_string())
            .collect::<Vec<String>>(),
    );

    // Track whether a selection was explicitly confirmed
    let tag_submitted = Rc::new(AtomicBool::new(false));

    // ── Populate & Filter List ───────────────────────────────────────────────
    let populate = {
        let all_tags = all_tags.clone();
        let list_box = list_box.clone();
        let stack = stack.clone();

        Rc::new(move |query: &str| {
            while let Some(child) = list_box.first_child() {
                list_box.remove(&child);
            }

            let query_clean = query.trim().trim_start_matches('#').to_lowercase();
            let mut matches = 0;

            for tag in all_tags.iter() {
                if !query_clean.is_empty() && !tag.to_lowercase().contains(&query_clean) {
                    continue;
                }
                matches += 1;

                let row = gtk::ListBoxRow::new();
                let row_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(12)
                    .margin_start(14)
                    .margin_end(14)
                    .margin_top(8)
                    .margin_bottom(8)
                    .build();

                let icon = gtk::Image::builder()
                    .icon_name("tag-symbolic")
                    .pixel_size(16)
                    .opacity(0.7)
                    .build();

                let label = gtk::Label::builder()
                    .label(format!("#{}", tag))
                    .halign(gtk::Align::Start)
                    .hexpand(true)
                    .build();

                row_box.append(&icon);
                row_box.append(&label);
                row.set_child(Some(&row_box));
                list_box.append(&row);
            }

            if matches == 0 {
                stack.set_visible_child_name("empty");
            } else {
                stack.set_visible_child_name("list");
                if let Some(first) = list_box.row_at_index(0) {
                    list_box.select_row(Some(&first));
                }
            }
        })
    };

    populate("");

    // Live search typing
    {
        let populate = populate.clone();
        search_entry.connect_search_changed(move |entry| {
            populate(&entry.text());
        });
    }

    // Live Update Helper
    let update_filter = {
        let sender = sender.clone();
        Rc::new(move |selected_text: String| {
            let tag_query = format!("#{}", selected_text.trim_start_matches('#'));
            sender.input(AppMsg::SwitchHeader(constants::VIEW_SEARCH.to_string()));
            sender.input(AppMsg::UpdateFilter(tag_query));
        })
    };

    // Live Selection Presentation on cursor change
    {
        let update_filter = update_filter.clone();
        list_box.connect_selected_rows_changed(move |box_| {
            if let Some(selected_row) = box_.selected_row() {
                if let Some(row_box) = selected_row.child().and_downcast::<gtk::Box>() {
                    if let Some(lbl) = row_box.last_child().and_downcast::<gtk::Label>() {
                        update_filter(lbl.text().to_string());
                    }
                }
            }
        });
    }

    // Row Click / Activate: Confirms selection and closes dialog
    {
        let dialog = dialog.clone();
        let update_filter = update_filter.clone();
        let tag_submitted = tag_submitted.clone();
        list_box.connect_row_activated(move |_, row| {
            if let Some(row_box) = row.child().and_downcast::<gtk::Box>() {
                if let Some(lbl) = row_box.last_child().and_downcast::<gtk::Label>() {
                    tag_submitted.store(true, Ordering::SeqCst);
                    update_filter(lbl.text().to_string());
                    dialog.close();
                }
            }
        });
    }

    // Enter Key pressed in Search Entry: Confirms selection and closes dialog
    {
        let dialog = dialog.clone();
        let list_box = list_box.clone();
        let update_filter = update_filter.clone();
        let tag_submitted = tag_submitted.clone();
        search_entry.connect_activate(move |_| {
            if let Some(selected_row) = list_box.selected_row() {
                if let Some(row_box) = selected_row.child().and_downcast::<gtk::Box>() {
                    if let Some(lbl) = row_box.last_child().and_downcast::<gtk::Label>() {
                        tag_submitted.store(true, Ordering::SeqCst);
                        update_filter(lbl.text().to_string());
                    }
                }
            }
            dialog.close();
        });
    }

    // Recover previous view on close if dismissed without confirming
    {
        let sender = sender.clone();
        let tag_submitted = tag_submitted.clone();
        dialog.connect_close_request(move |_| {
            if !tag_submitted.load(Ordering::SeqCst) {
                sender.input(AppMsg::CancelContentSearch);
            }
            gtk::glib::Propagation::Proceed
        });
    }

    // Key controller: Escape to close, Up/Down arrows to move list selection from entry
    {
        let d = dialog.clone();
        let list_box = list_box.clone();
        let key_ctrl = gtk::EventControllerKey::new();
        key_ctrl.connect_key_pressed(move |_, keyval, _, _| match keyval {
            adw::gdk::Key::Escape => {
                d.close();
                gtk::glib::Propagation::Stop
            }
            adw::gdk::Key::Down => {
                if let Some(current) = list_box.selected_row() {
                    let next_idx = current.index() + 1;
                    if let Some(next_row) = list_box.row_at_index(next_idx) {
                        list_box.select_row(Some(&next_row));
                    }
                }
                gtk::glib::Propagation::Stop
            }
            adw::gdk::Key::Up => {
                if let Some(current) = list_box.selected_row() {
                    let idx = current.index();
                    if idx > 0 {
                        if let Some(prev_row) = list_box.row_at_index(idx - 1) {
                            list_box.select_row(Some(&prev_row));
                        }
                    }
                }
                gtk::glib::Propagation::Stop
            }
            _ => gtk::glib::Propagation::Proceed,
        });
        search_entry.add_controller(key_ctrl);
    }

    let search_focus = search_entry.clone();
    dialog.connect_map(move |_| {
        search_focus.grab_focus();
    });

    dialog.present();
}
