use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use super::header::panel_header;
use super::resize::resizable_panel;
use super::spec::PanelSpec;
use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};

/// Builds and returns the unified tag panel navigator.
pub fn build_tag_panel(
    available_tags: Vec<String>,
    initial_width: i32,
    sender: AsyncComponentSender<FluxApp>,
) -> gtk::Box {
    let spec = PanelSpec::tag().with_initial(initial_width);

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(spec.effective())
        .hexpand(false)
        .build();
    panel.add_css_class("sidebar");

    {
        let s = sender.clone();
        let header = panel_header(None, &tr("Tag Navigator"), &[], move || {
            s.input(AppMsg::ToggleTagPanel)
        });
        panel.append(&header);
    }

    let all_known_tags = Rc::new(RefCell::new(
        available_tags
            .into_iter()
            .map(|t| t.trim_start_matches('#').to_lowercase())
            .filter(|t| !t.is_empty())
            .collect::<BTreeSet<String>>(),
    ));

    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .margin_top(4)
        .margin_bottom(12)
        .vexpand(true)
        .build();

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(tr("Search or type new tag…"))
        .hexpand(true)
        .css_classes(["tag-search-entry"])
        .build();
    content_box.append(&search_entry);

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

    let sender_for_resize = sender.clone();
    resizable_panel(&spec, &panel, move |w| {
        sender_for_resize.input(AppMsg::SetTagPanelWidth(w));
    })
}
