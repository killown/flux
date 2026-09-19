use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::rc::Rc;

use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;

/// Builds and returns a clean, streamlined right tag navigator sidebar panel.
pub fn build_tag_panel(
    available_tags: Vec<String>,
    sender: AsyncComponentSender<FluxApp>,
) -> gtk::Box {
    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    panel.add_css_class("sidebar");

    // ── Header ───────────────────────────────────────────────────────────────
    let header_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .margin_bottom(8)
        .build();

    let title_label = gtk::Label::builder()
        .label(tr("Tag Navigator"))
        .css_classes(["heading"])
        .hexpand(true)
        .xalign(0.0)
        .build();

    let close_btn = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .tooltip_text(tr("Cancel"))
        .build();

    {
        let s = sender.clone();
        close_btn.connect_clicked(move |_| {
            s.input(AppMsg::ToggleTagPanel);
        });
    }

    header_box.append(&title_label);
    header_box.append(&close_btn);
    panel.append(&header_box);

    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .margin_top(4)
        .margin_bottom(12)
        .vexpand(true)
        .build();

    // ── Search Entry ─────────────────────────────────────────────────────────
    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(tr("Type tag name…"))
        .hexpand(true)
        .build();
    content_box.append(&search_entry);

    // ── Tag List ─────────────────────────────────────────────────────────────
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

    content_box.append(&scroll);
    panel.append(&content_box);

    let all_tags = Rc::new(
        available_tags
            .into_iter()
            .map(|t| t.trim_start_matches('#').to_string())
            .collect::<Vec<String>>(),
    );

    // ── Populate & Filter List ───────────────────────────────────────────────
    let populate = {
        let all_tags = all_tags.clone();
        let list_box = list_box.clone();
        let sender = sender.clone();

        Rc::new(move |query: &str| {
            while let Some(child) = list_box.first_child() {
                list_box.remove(&child);
            }

            let query_clean = query.trim().trim_start_matches('#').to_lowercase();

            for tag in all_tags.iter() {
                if !query_clean.is_empty() && !tag.to_lowercase().contains(&query_clean) {
                    continue;
                }

                let row = gtk::ListBoxRow::new();

                let row_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(8)
                    .margin_start(14)
                    .margin_end(8)
                    .margin_top(6)
                    .margin_bottom(6)
                    .hexpand(true)
                    .build();

                let label = gtk::Label::builder()
                    .label(format!("#{}", tag))
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Center)
                    .hexpand(true)
                    .build();

                let bookmark_btn = gtk::Button::builder()
                    .icon_name("bookmark-new-symbolic")
                    .css_classes(["flat", "circular"])
                    .valign(gtk::Align::Center)
                    .tooltip_text(tr("Pin to Sidebar"))
                    .build();

                {
                    let s = sender.clone();
                    let tag_name = tag.clone();
                    bookmark_btn.connect_clicked(move |_| {
                        s.input(AppMsg::AddTagToSidebar(tag_name.clone()));
                    });
                }

                row_box.append(&label);
                row_box.append(&bookmark_btn);

                row.set_child(Some(&row_box));
                list_box.append(&row);
            }

            if let Some(first) = list_box.row_at_index(0) {
                list_box.select_row(Some(&first));
            }
        })
    };

    populate("");

    {
        let populate = populate.clone();
        search_entry.connect_search_changed(move |entry| {
            populate(&entry.text());
        });
    }

    let update_filter = {
        let sender = sender.clone();
        Rc::new(move |selected_text: String| {
            let tag_query = format!("#{}", selected_text.trim_start_matches('#'));
            sender.input(AppMsg::SwitchHeader(constants::VIEW_SEARCH.to_string()));
            sender.input(AppMsg::UpdateFilter(tag_query));
        })
    };

    {
        let update_filter = update_filter.clone();
        list_box.connect_row_activated(move |_, row| {
            if let Some(row_box) = row.child().and_downcast::<gtk::Box>() {
                if let Some(lbl) = row_box.first_child().and_downcast::<gtk::Label>() {
                    update_filter(lbl.text().to_string());
                }
            }
        });
    }

    // ── Keyboard Navigation ──────────────────────────────────────────────────
    let key_ctrl = gtk::EventControllerKey::new();
    {
        let s = sender.clone();
        let list_box = list_box.clone();
        key_ctrl.connect_key_pressed(move |_, keyval, _, _| match keyval {
            adw::gdk::Key::Escape => {
                s.input(AppMsg::ToggleTagPanel);
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
    }
    search_entry.add_controller(key_ctrl);

    let first_focus = search_entry.clone();
    panel.connect_map(move |_| {
        first_focus.grab_focus();
    });

    panel
}
