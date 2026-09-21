use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};

/// Builds and returns a fused tag panel: collapsible editor at top, navigator below.
pub fn build_tag_panel(
    available_tags: Vec<String>,
    initial_width: i32,
    sender: AsyncComponentSender<FluxApp>,
) -> gtk::Box {
    let effective_width = if initial_width <= 0 {
        350
    } else {
        initial_width.clamp(250, 800)
    };

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(effective_width)
        .hexpand(false)
        .build();

    panel.add_css_class("sidebar");

    // ── Drag Handle (Left Edge) using a decoupled EventControllerMotion ─────────
    // Using root coordinates instead of local widget delta avoids coordinate shifts
    let resize_handle = gtk::Separator::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["sidebar-resize-handle"])
        .cursor(&gtk::gdk::Cursor::from_name("col-resize", None).unwrap())
        .build();

    let drag_gesture = gtk::GestureDrag::new();
    let start_width = std::rc::Rc::new(std::cell::Cell::new(effective_width));
    let start_root_x = std::rc::Rc::new(std::cell::Cell::new(0.0));

    {
        let panel_weak = panel.downgrade();
        let start_width_c = start_width.clone();
        let start_root_x_c = start_root_x.clone();

        drag_gesture.connect_drag_begin(move |gesture, x, _| {
            if let Some(p) = panel_weak.upgrade() {
                start_width_c.set(p.width());
                // Translate the initial click point to root/window coordinate space
                // Root coordinates remain completely static while children resize!
                if let Some(root) = p.root() {
                    if let Some(handle) = gesture.widget() {
                        let (rx, _) = handle
                            .translate_coordinates(&root, x, 0.0)
                            .unwrap_or((x, 0.0));
                        start_root_x_c.set(rx);
                    }
                }
            }
        });
    }

    {
        let panel_weak = panel.downgrade();
        let start_width_c = start_width.clone();
        let start_root_x_c = start_root_x.clone();

        drag_gesture.connect_drag_update(move |gesture, _, _| {
            if let Some(p) = panel_weak.upgrade() {
                if let Some(root) = p.root() {
                    if let Some(handle) = gesture.widget() {
                        // Query the current point and translate directly to root coordinates
                        if let Some((curr_x, _)) = gesture.point(None) {
                            if let Some((curr_root_x, _)) =
                                handle.translate_coordinates(&root, curr_x, 0.0)
                            {
                                // Real delta = how much the pointer moved in global window space
                                let delta_x = curr_root_x - start_root_x_c.get();
                                let new_w = (start_width_c.get() - delta_x as i32).clamp(250, 800);

                                if p.width_request() != new_w {
                                    p.set_width_request(new_w);
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let panel_weak = panel.downgrade();
        let s = sender.clone();
        drag_gesture.connect_drag_end(move |_, _, _| {
            if let Some(p) = panel_weak.upgrade() {
                let final_width = p.width().clamp(250, 800);
                p.set_width_request(final_width);
                s.input(AppMsg::SetTagPanelWidth(final_width));
            }
        });
    }

    resize_handle.add_controller(drag_gesture);

    let root_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(0)
        .hexpand(false)
        .halign(gtk::Align::End)
        .build();

    root_container.append(&resize_handle);
    root_container.append(&panel);

    let all_known_tags = Rc::new(RefCell::new(
        available_tags
            .into_iter()
            .map(|t| t.trim_start_matches('#').to_lowercase())
            .filter(|t| !t.is_empty())
            .collect::<BTreeSet<String>>(),
    ));

    let active_selected_tags = Rc::new(RefCell::new(BTreeSet::<String>::new()));

    // ── Header: Title, Toggle Edit Picker, Close ─────────────────────────────
    let header_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
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

    let edit_toggle_btn = gtk::ToggleButton::builder()
        .icon_name("tag-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .tooltip_text(tr("Edit tags for selection"))
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
    header_box.append(&edit_toggle_btn);
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

    // ── Collapsible Top Picker (Revealer) ────────────────────────────────────
    let picker_revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .reveal_child(false)
        .build();

    let picker_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_bottom(6)
        .css_classes(["card"])
        .margin_start(2)
        .margin_end(2)
        .build();

    let picker_header = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(8)
        .margin_end(8)
        .margin_top(8)
        .build();

    let picker_title = gtk::Label::builder()
        .label(tr("Edit Tags"))
        .css_classes(["caption", "heading"])
        .hexpand(true)
        .xalign(0.0)
        .build();

    let picker_apply_btn = gtk::Button::builder()
        .label(tr("Apply"))
        .css_classes(["suggested-action", "pill"])
        .valign(gtk::Align::Center)
        .build();

    picker_header.append(&picker_title);
    picker_header.append(&picker_apply_btn);
    picker_container.append(&picker_header);

    let picker_flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(3)
        .min_children_per_line(1)
        .row_spacing(6)
        .column_spacing(6)
        .homogeneous(false)
        .valign(gtk::Align::Start)
        .margin_start(8)
        .margin_end(8)
        .margin_bottom(8)
        .build();

    let picker_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .overlay_scrolling(false)
        .max_content_height(160)
        .propagate_natural_height(true)
        .child(&picker_flow)
        .build();

    picker_container.append(&picker_scroll);
    picker_revealer.set_child(Some(&picker_container));
    content_box.append(&picker_revealer);

    {
        let rev = picker_revealer.clone();
        edit_toggle_btn.connect_toggled(move |btn| {
            rev.set_reveal_child(btn.is_active());
        });
    }

    // ── Search & Filter Entry ────────────────────────────────────────────────
    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(tr("Search or type new tag…"))
        .hexpand(true)
        .build();
    content_box.append(&search_entry);

    // ── Navigator Tag List ───────────────────────────────────────────────────
    let list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(["boxed-list"])
        .margin_end(4)
        .build();

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .overlay_scrolling(false)
        .propagate_natural_width(false)
        .hexpand(false)
        .vexpand(true)
        .child(&list_box)
        .build();

    content_box.append(&scroll);
    panel.append(&content_box);

    // ── Refresh & Population Closures ────────────────────────────────────────
    let refresh_picker_chips = {
        let all_known_tags = all_known_tags.clone();
        let active_selected_tags = active_selected_tags.clone();
        let picker_flow = picker_flow.clone();

        Rc::new(move || {
            while let Some(child) = picker_flow.first_child() {
                picker_flow.remove(&child);
            }

            let known = all_known_tags.borrow();
            for tag_name in known.iter() {
                let is_active = active_selected_tags.borrow().contains(tag_name);

                let chip = gtk::ToggleButton::builder()
                    .label(format!("#{}", tag_name))
                    .active(is_active)
                    .css_classes(["pill"])
                    .build();

                {
                    let tag = tag_name.clone();
                    let active_selected_tags = active_selected_tags.clone();
                    chip.connect_toggled(move |btn| {
                        if btn.is_active() {
                            active_selected_tags.borrow_mut().insert(tag.clone());
                        } else {
                            active_selected_tags.borrow_mut().remove(&tag);
                        }
                    });
                }

                picker_flow.append(&chip);
            }
        })
    };

    let populate_list = {
        let all_known_tags = all_known_tags.clone();
        let active_selected_tags = active_selected_tags.clone();
        let list_box = list_box.clone();
        let sender = sender.clone();
        let refresh_picker_chips = refresh_picker_chips.clone();

        Rc::new(move |query: &str| {
            while let Some(child) = list_box.first_child() {
                list_box.remove(&child);
            }

            let query_clean = query.trim().trim_start_matches('#').to_lowercase();
            let known = all_known_tags.borrow();

            for tag in known.iter() {
                if !query_clean.is_empty() && !tag.contains(&query_clean) {
                    continue;
                }

                let row = gtk::ListBoxRow::new();
                let row_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(8)
                    .margin_start(12)
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

                let delete_btn = gtk::Button::builder()
                    .icon_name("window-close-symbolic")
                    .css_classes(["flat", "circular"])
                    .valign(gtk::Align::Center)
                    .tooltip_text(tr("Delete tag everywhere"))
                    .build();

                {
                    let s = sender.clone();
                    let tag_name = tag.clone();
                    let all_known_tags = all_known_tags.clone();
                    let active_selected_tags = active_selected_tags.clone();
                    let list_box = list_box.clone();
                    let row = row.clone();
                    let refresh_picker = refresh_picker_chips.clone();

                    delete_btn.connect_clicked(move |btn| {
                        let toplevel = btn.root().and_downcast::<gtk::Window>();
                        let heading = tr("Delete Tag");
                        let dialog = gtk::MessageDialog::new(
                            toplevel.as_ref(),
                            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
                            gtk::MessageType::Question,
                            gtk::ButtonsType::None,
                            &heading,
                        );

                        dialog.set_secondary_text(Some(&format!(
                            "{}: \"#{}\"?",
                            tr("Are you sure you want to delete this tag globally"),
                            tag_name
                        )));

                        dialog.add_button(&tr("Cancel"), gtk::ResponseType::Cancel);
                        let del_btn = dialog.add_button(&tr("Delete"), gtk::ResponseType::Ok);
                        del_btn.style_context().add_class("destructive-action");
                        dialog.set_default_response(gtk::ResponseType::Cancel);

                        let s = s.clone();
                        let tag_name = tag_name.clone();
                        let all_known_tags = all_known_tags.clone();
                        let active_selected_tags = active_selected_tags.clone();
                        let list_box = list_box.clone();
                        let row = row.clone();
                        let refresh_picker = refresh_picker.clone();

                        dialog.connect_response(move |dlg, response| {
                            if response == gtk::ResponseType::Ok {
                                all_known_tags.borrow_mut().remove(&tag_name);
                                active_selected_tags.borrow_mut().remove(&tag_name);
                                list_box.remove(&row);
                                refresh_picker();
                                s.input(AppMsg::DeleteTagGlobally(tag_name.clone()));
                            }
                            dlg.close();
                        });

                        dialog.present();
                    });
                }

                row_box.append(&label);
                row_box.append(&bookmark_btn);
                row_box.append(&delete_btn);

                row.set_child(Some(&row_box));
                list_box.append(&row);
            }

            list_box.select_row(None::<&gtk::ListBoxRow>);
        })
    };

    refresh_picker_chips();
    populate_list("");

    {
        let list_box_clean = list_box.clone();
        panel.connect_map(move |_| {
            let lb = list_box_clean.clone();
            gtk::glib::idle_add_local_once(move || {
                lb.select_row(None::<&gtk::ListBoxRow>);
            });
        });
    }

    // ── Apply Button for Top Picker ──────────────────────────────────────────
    {
        let active_selected_tags = active_selected_tags.clone();
        let s = sender.clone();
        let rev = picker_revealer.clone();
        let toggle = edit_toggle_btn.clone();

        picker_apply_btn.connect_clicked(move |_| {
            let tags: Vec<String> = active_selected_tags.borrow().iter().cloned().collect();
            s.input(AppMsg::ApplyTagsToSelection(tags));
            rev.set_reveal_child(false);
            toggle.set_active(false);
        });
    }

    // ── Live Search & Creation on Enter ──────────────────────────────────────
    {
        let populate_list = populate_list.clone();
        search_entry.connect_search_changed(move |entry| {
            populate_list(&entry.text());
        });
    }

    {
        let all_known_tags = all_known_tags.clone();
        let active_selected_tags = active_selected_tags.clone();
        let search_entry_clone = search_entry.clone();
        let populate_list = populate_list.clone();
        let refresh_picker = refresh_picker_chips.clone();

        search_entry.connect_activate(move |_| {
            let clean = search_entry_clone
                .text()
                .trim()
                .trim_start_matches('#')
                .to_lowercase();

            if !clean.is_empty() {
                all_known_tags.borrow_mut().insert(clean.clone());
                active_selected_tags.borrow_mut().insert(clean);
                search_entry_clone.set_text("");
                refresh_picker();
                populate_list("");
            }
        });
    }

    // ── Navigation Click Action ──────────────────────────────────────────────
    {
        let s = sender.clone();
        list_box.connect_row_activated(move |_, row| {
            if let Some(row_box) = row.child().and_downcast::<gtk::Box>() {
                if let Some(lbl) = row_box.first_child().and_downcast::<gtk::Label>() {
                    let tag_query = format!("#{}", lbl.text().trim_start_matches('#'));
                    s.input(AppMsg::UpdateFilter(tag_query));
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
                } else if let Some(first_row) = list_box.row_at_index(0) {
                    list_box.select_row(Some(&first_row));
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

    root_container
}
