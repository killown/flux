use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};

/// Constructs and presents the tag editor dialog centered on the application window.
pub fn show_tag_picker(
    parent: &impl IsA<gtk::Widget>,
    paths: Vec<PathBuf>,
    initial_tags: Vec<String>,
    available_tags: Vec<String>,
    sender: AsyncComponentSender<FluxApp>,
) {
    if paths.is_empty() {
        return;
    }

    let parent_window = parent.root().and_downcast::<gtk::Window>();

    let dialog = adw::Window::builder()
        .title(tr("Edit Tags").as_str())
        .modal(true)
        .default_width(420)
        .default_height(480)
        .resizable(false)
        .build();

    if let Some(ref win) = parent_window {
        dialog.set_transient_for(Some(win));
    }

    let current_selected = Rc::new(RefCell::new(
        initial_tags
            .into_iter()
            .map(|t| t.trim_start_matches('#').to_lowercase())
            .filter(|t| !t.is_empty())
            .collect::<BTreeSet<String>>(),
    ));

    let all_known_tags = Rc::new(RefCell::new({
        let mut set = available_tags
            .into_iter()
            .map(|t| t.trim_start_matches('#').to_lowercase())
            .filter(|t| !t.is_empty())
            .collect::<BTreeSet<String>>();
        for tag in current_selected.borrow().iter() {
            set.insert(tag.clone());
        }
        set
    }));

    let main_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);

    let title_label = gtk::Label::builder()
        .label(tr("Edit Tags").as_str())
        .css_classes(["title-4", "heading"])
        .build();
    header.set_title_widget(Some(&title_label));

    let cancel_btn = gtk::Button::builder().label(tr("Cancel").as_str()).build();
    let apply_btn = gtk::Button::builder()
        .label(tr("Apply").as_str())
        .css_classes(["suggested-action"])
        .build();

    header.pack_start(&cancel_btn);
    header.pack_end(&apply_btn);
    main_box.append(&header);

    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(20)
        .margin_end(20)
        .vexpand(true)
        .build();

    // ── Search & Creation Entry ──────────────────────────────────────────────
    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(tr("Search or type new tag…").as_str())
        .hexpand(true)
        .build();

    content_box.append(&search_entry);

    // ── Tag Flow List ────────────────────────────────────────────────────────
    let flow_box = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(3)
        .min_children_per_line(1)
        .row_spacing(8)
        .column_spacing(8)
        .homogeneous(false)
        .valign(gtk::Align::Start)
        .build();

    let empty_status = adw::StatusPage::builder()
        .icon_name("tag-symbolic")
        .title(tr("No Tags Found").as_str())
        .description(tr("Press Enter to create and assign this tag").as_str())
        .vexpand(true)
        .build();

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vexpand(true)
        .build();

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&flow_box)
        .build();

    stack.add_named(&scroll, Some("list"));
    stack.add_named(&empty_status, Some("empty"));
    stack.set_visible_child_name("list");

    content_box.append(&stack);
    main_box.append(&content_box);
    dialog.set_content(Some(&main_box));

    // ── Tag Chip Renderer ───────────────────────────────────────────────────
    let populate_tags = {
        let all_known_tags = all_known_tags.clone();
        let current_selected = current_selected.clone();
        let flow_box = flow_box.clone();
        let stack = stack.clone();
        let sender = sender.clone();

        Rc::new(move |filter_query: &str| {
            while let Some(child) = flow_box.first_child() {
                flow_box.remove(&child);
            }

            let query = filter_query.trim().to_lowercase();
            let known = all_known_tags.borrow();
            let mut visible_count = 0;

            for tag_name in known.iter() {
                if !query.is_empty() && !tag_name.contains(&query) {
                    continue;
                }
                visible_count += 1;

                // GNOME HIG-style pill with attached close icon
                let chip_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(0)
                    .css_classes(["linked", "card"])
                    .margin_top(2)
                    .margin_bottom(2)
                    .build();

                let is_active = current_selected.borrow().contains(tag_name);
                let toggle_btn = gtk::ToggleButton::builder()
                    .label(format!("#{}", tag_name))
                    .active(is_active)
                    .css_classes(["flat"])
                    .hexpand(true)
                    .build();

                let delete_btn = gtk::Button::builder()
                    .icon_name("window-close-symbolic")
                    .tooltip_text(tr("Delete tag everywhere").as_str())
                    .css_classes(["flat", "circular"])
                    .build();

                {
                    let tag_name = tag_name.clone();
                    let current_selected = current_selected.clone();
                    toggle_btn.connect_toggled(move |btn| {
                        if btn.is_active() {
                            current_selected.borrow_mut().insert(tag_name.clone());
                        } else {
                            current_selected.borrow_mut().remove(&tag_name);
                        }
                    });
                }

                {
                    let tag_name = tag_name.clone();
                    let all_known_tags = all_known_tags.clone();
                    let current_selected = current_selected.clone();
                    let flow_box = flow_box.clone();
                    let chip_box = chip_box.clone();
                    let s = sender.clone();

                    delete_btn.connect_clicked(move |_| {
                        all_known_tags.borrow_mut().remove(&tag_name);
                        current_selected.borrow_mut().remove(&tag_name);
                        flow_box.remove(&chip_box);
                        s.input(AppMsg::DeleteTagGlobally(tag_name.clone()));
                    });
                }

                chip_box.append(&toggle_btn);
                chip_box.append(&delete_btn);
                flow_box.append(&chip_box);
            }

            if visible_count == 0 {
                stack.set_visible_child_name("empty");
            } else {
                stack.set_visible_child_name("list");
            }
        })
    };

    populate_tags("");

    // Live search filter trigger
    {
        let populate_tags = populate_tags.clone();
        search_entry.connect_search_changed(move |entry| {
            populate_tags(&entry.text());
        });
    }

    // Creating new tag on Enter
    {
        let all_known_tags = all_known_tags.clone();
        let current_selected = current_selected.clone();
        let search_entry_clone = search_entry.clone();
        let populate_tags = populate_tags.clone();

        search_entry.connect_activate(move |_| {
            let clean = search_entry_clone
                .text()
                .trim()
                .trim_start_matches('#')
                .to_lowercase();

            if !clean.is_empty() {
                all_known_tags.borrow_mut().insert(clean.clone());
                current_selected.borrow_mut().insert(clean);
                search_entry_clone.set_text("");
                populate_tags("");
            }
        });
    }

    let tags_submitted = std::rc::Rc::new(std::cell::Cell::new(false));

    // Header buttons bindings
    {
        let d = dialog.clone();
        cancel_btn.connect_clicked(move |_| d.close());
    }

    {
        let d = dialog.clone();
        let current_selected = current_selected.clone();
        let paths = paths.clone();
        let s = sender.clone();
        let submitted = tags_submitted.clone();

        apply_btn.connect_clicked(move |_| {
            submitted.set(true);
            let tags: Vec<String> = current_selected.borrow().iter().cloned().collect();
            for path in &paths {
                s.input(AppMsg::SetFileTags {
                    path: path.clone(),
                    tags: tags.clone(),
                });
            }
            d.close();
        });
    }

    let search_entry_focus = search_entry.clone();
    dialog.connect_map(move |_| {
        search_entry_focus.grab_focus();
    });

    // Recover current folder on close/cancel if inside search://
    {
        let sender = sender.clone();
        let submitted = tags_submitted.clone();
        let fallback_parent = paths
            .first()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());

        dialog.connect_close_request(move |_| {
            if !submitted.get() {
                if let Some(target) = fallback_parent.as_ref() {
                    sender.input(AppMsg::CancelContentSearch);
                    sender.input(AppMsg::Navigate(target.clone()));
                }
            }
            gtk::glib::Propagation::Proceed
        });
    }

    // Escape closes dialog
    {
        let d = dialog.clone();
        let key_ctrl = gtk::EventControllerKey::new();
        key_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == adw::gdk::Key::Escape {
                d.close();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl);
    }

    dialog.present();
}
