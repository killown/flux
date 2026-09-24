use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};

/// Builds and returns the unified tag panel navigator.
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
        .build();

    let drag_gesture = gtk::GestureDrag::new();
    let start_width = std::rc::Rc::new(std::cell::Cell::new(effective_width));
    let start_root_x = std::rc::Rc::new(std::cell::Cell::new(0.0));
    let hover_timer = std::rc::Rc::new(std::cell::Cell::new(None::<gtk::glib::SourceId>));
    let is_ready = std::rc::Rc::new(std::cell::Cell::new(false));

    let motion_ctrl = gtk::EventControllerMotion::new();

    {
        let timer_c = hover_timer.clone();
        let ready_c = is_ready.clone();
        motion_ctrl.connect_enter(move |ctrl, _, _| {
            if let Some(id) = timer_c.take() {
                id.remove();
            }
            ready_c.set(false);

            let ctrl_weak = ctrl.downgrade();
            let timer_inner = timer_c.clone();
            let ready_inner = ready_c.clone();

            let id = gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(100),
                move || {
                    timer_inner.set(None);
                    ready_inner.set(true);
                    if let Some(c) = ctrl_weak.upgrade() {
                        if let Some(widget) = c.widget() {
                            widget.set_cursor_from_name(Some("col-resize"));
                        }
                    }
                },
            );
            timer_c.set(Some(id));
        });
    }

    {
        let timer_c = hover_timer;
        let ready_c = is_ready.clone();
        motion_ctrl.connect_leave(move |ctrl| {
            if let Some(id) = timer_c.take() {
                id.remove();
            }
            ready_c.set(false);
            if let Some(widget) = ctrl.widget() {
                widget.set_cursor(None);
            }
        });
    }

    resize_handle.add_controller(motion_ctrl);

    {
        let panel_weak = panel.downgrade();
        let start_width_c = start_width.clone();
        let start_root_x_c = start_root_x.clone();
        let ready_c = is_ready.clone();

        drag_gesture.connect_drag_begin(move |gesture, x, _| {
            if !ready_c.get() {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
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
        let ready_c = is_ready.clone();

        drag_gesture.connect_drag_update(move |gesture, _, _| {
            if !ready_c.get() {
                return;
            }
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
        let ready_c = is_ready;

        drag_gesture.connect_drag_end(move |gesture, _, _| {
            if !ready_c.get() {
                return;
            }
            if let Some(p) = panel_weak.upgrade() {
                let final_width = p.width().clamp(250, 800);
                p.set_width_request(final_width);
                s.input(AppMsg::SetTagPanelWidth(final_width));
            }
            if let Some(widget) = gesture.widget() {
                widget.set_cursor(None);
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

    // ── Header: Title, Close ─────────────────────────────────────────────────
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

    // ── Population Closure ───────────────────────────────────────────────────
    let populate_list = {
        let all_known_tags = all_known_tags.clone();
        let list_box = list_box.clone();
        let sender = sender.clone();

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

                // ── Popover Context Menu ─────────────────────────────────────
                let menu_button = gtk::MenuButton::builder()
                    .icon_name("view-more-symbolic")
                    .css_classes(["flat", "circular"])
                    .valign(gtk::Align::Center)
                    .tooltip_text(tr("Tag options"))
                    .build();

                let popover = gtk::Popover::new();
                let menu_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .spacing(4)
                    .margin_top(6)
                    .margin_bottom(6)
                    .margin_start(6)
                    .margin_end(6)
                    .build();

                // Apply Tag
                let apply_item = gtk::Button::builder().css_classes(["flat"]).build();
                let apply_content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                apply_content.append(&gtk::Image::from_icon_name("list-add-symbolic"));
                apply_content.append(&gtk::Label::new(Some(&tr("Apply to selection"))));
                apply_item.set_child(Some(&apply_content));

                {
                    let s = sender.clone();
                    let tag_name = tag.clone();
                    let pop = popover.clone();
                    apply_item.connect_clicked(move |_| {
                        pop.popdown();
                        s.input(AppMsg::ApplyTagsToSelection(vec![tag_name.clone()]));
                    });
                }
                menu_box.append(&apply_item);

                // Remove Tag
                let remove_item = gtk::Button::builder().css_classes(["flat"]).build();
                let remove_content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                remove_content.append(&gtk::Image::from_icon_name("list-remove-symbolic"));
                remove_content.append(&gtk::Label::new(Some(&tr("Remove from selection"))));
                remove_item.set_child(Some(&remove_content));

                {
                    let s = sender.clone();
                    let tag_name = tag.clone();
                    let pop = popover.clone();
                    remove_item.connect_clicked(move |_| {
                        pop.popdown();
                        s.input(AppMsg::RemoveTagFromSelection(tag_name.clone()));
                    });
                }
                menu_box.append(&remove_item);

                // Pin to Sidebar
                let pin_item = gtk::Button::builder().css_classes(["flat"]).build();
                let pin_content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                pin_content.append(&gtk::Image::from_icon_name("bookmark-new-symbolic"));
                pin_content.append(&gtk::Label::new(Some(&tr("Pin to Sidebar"))));
                pin_item.set_child(Some(&pin_content));

                {
                    let s = sender.clone();
                    let tag_name = tag.clone();
                    let pop = popover.clone();
                    pin_item.connect_clicked(move |_| {
                        pop.popdown();
                        s.input(AppMsg::AddTagToSidebar(tag_name.clone()));
                    });
                }
                menu_box.append(&pin_item);

                menu_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

                // Delete Tag Globally
                let delete_item = gtk::Button::builder()
                    .css_classes(["flat", "destructive-action"])
                    .build();
                let delete_content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                delete_content.append(&gtk::Image::from_icon_name("user-trash-symbolic"));
                delete_content.append(&gtk::Label::new(Some(&tr("Delete tag globally"))));
                delete_item.set_child(Some(&delete_content));

                {
                    let s = sender.clone();
                    let tag_name = tag.clone();
                    let all_known_tags = all_known_tags.clone();
                    let list_box = list_box.clone();
                    let row = row.clone();
                    let pop = popover.clone();

                    delete_item.connect_clicked(move |btn| {
                        pop.popdown();
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
                        let list_box = list_box.clone();
                        let row = row.clone();

                        dialog.connect_response(move |dlg, response| {
                            if response == gtk::ResponseType::Ok {
                                all_known_tags.borrow_mut().remove(&tag_name);
                                list_box.remove(&row);
                                s.input(AppMsg::DeleteTagGlobally(tag_name.clone()));
                            }
                            dlg.close();
                        });

                        dialog.present();
                    });
                }
                menu_box.append(&delete_item);

                popover.set_child(Some(&menu_box));
                menu_button.set_popover(Some(&popover));

                row_box.append(&label);
                row_box.append(&menu_button);

                row.set_child(Some(&row_box));
                list_box.append(&row);
            }

            list_box.select_row(None::<&gtk::ListBoxRow>);
        })
    };

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

    // ── Live Search & Creation on Enter ──────────────────────────────────────
    {
        let populate_list = populate_list.clone();
        search_entry.connect_search_changed(move |entry| {
            populate_list(&entry.text());
        });
    }

    {
        let all_known_tags = all_known_tags.clone();
        let search_entry_clone = search_entry.clone();
        let populate_list = populate_list.clone();

        search_entry.connect_activate(move |_| {
            let clean = search_entry_clone
                .text()
                .trim()
                .trim_start_matches('#')
                .to_lowercase();

            if !clean.is_empty() {
                all_known_tags.borrow_mut().insert(clean.clone());
                search_entry_clone.set_text("");
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
