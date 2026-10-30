//! "New Action" dialog of the Contextual Actions panel.

use crate::domain::model::{AppMsg, FluxApp};
use crate::i18n::tr;
use crate::ui::menu::secondary::{category_mime_pattern, secondary_template_filename, split_mime};
use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::cell::RefCell;
use std::rc::Rc;

/// A labelled text entry row, matching the stacked rows of the menu editor.
fn entry_row(title: &str, value: &str) -> (adw::PreferencesRow, gtk::Entry) {
    let entry = gtk::Entry::builder()
        .text(value)
        .hexpand(true)
        .margin_top(4)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();
    let vbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_top(8)
        .margin_bottom(4)
        .build();
    vbox.append(
        &gtk::Label::builder()
            .label(title)
            .halign(gtk::Align::Start)
            .margin_start(12)
            .css_classes(["heading"])
            .build(),
    );
    vbox.append(&entry);
    let row = adw::PreferencesRow::builder().build();
    row.set_child(Some(&vbox));
    (row, entry)
}

/// Like [`entry_row`], plus a "Pick Icon" button that prefixes the text with a Nerd Font glyph.
fn icon_entry_row(
    title: &str,
    value: &str,
    dialog: &adw::Dialog,
) -> (adw::PreferencesRow, gtk::Entry) {
    let (row, entry) = entry_row(title, value);

    let icon_button = gtk::Button::builder()
        .label(tr("Pick Icon").as_str())
        .valign(gtk::Align::Center)
        .build();

    let entry_ref = entry.clone();
    let window_weak = dialog.downgrade();
    icon_button.connect_clicked(move |_| {
        let entry_ref = entry_ref.clone();
        let parent_win = window_weak.upgrade();
        crate::ui::dialog::menu_icon_picker::show_menu_icon_picker(
            parent_win.as_ref(),
            move |glyph| {
                let current = entry_ref.text().to_string();
                let cleaned = current
                    .chars()
                    .skip_while(|c| !c.is_alphanumeric() && *c != '(')
                    .collect::<String>();
                entry_ref.set_text(&format!(
                    "{}{}{}",
                    glyph,
                    crate::ui::dialog::menu_icon_picker::MENU_ICON_PADDING,
                    cleaned
                ));
            },
        );
    });

    // Move the entry out of its stacked box into a row next to the button.
    if let Some(vbox) = row.child().and_downcast::<gtk::Box>() {
        vbox.remove(&entry);
        let input_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        input_box.append(&entry);
        input_box.append(&icon_button);
        vbox.append(&input_box);
        entry.set_margin_start(0);
        entry.set_margin_end(0);
        input_box.set_margin_start(12);
        input_box.set_margin_end(12);
        input_box.set_margin_top(4);
        input_box.set_margin_bottom(8);
        entry.set_margin_top(0);
        entry.set_margin_bottom(0);
    }
    (row, entry)
}

/// The MIME text the dialog suggests for the chosen scope.
fn default_mime(mime: &str, category_scope: bool) -> String {
    if mime.is_empty() {
        return "all".to_string();
    }
    if category_scope {
        category_mime_pattern(split_mime(mime).0)
    } else {
        mime.to_string()
    }
}

impl FluxApp {
    /// Asks for label, command and scope, then sends `SaveContextualAction`.
    pub fn show_prompt_new_contextual_action(
        &self,
        mime: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("show_prompt_new_contextual_action");
        let dialog = adw::Dialog::new();
        dialog.set_title(tr("New Action").as_str());
        dialog.set_content_width(560);

        let outer = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        outer.append(&adw::HeaderBar::new());

        let scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .propagate_natural_height(true)
            .build();
        let page = adw::PreferencesPage::new();
        let g_id = adw::PreferencesGroup::builder()
            .title(tr("Identity").as_str())
            .build();
        let g_act = adw::PreferencesGroup::builder()
            .title(tr("Action").as_str())
            .build();

        // The scope only matters when the type has a subtype distinct from its category.
        let (category, subtype) = split_mime(&mime);
        let has_subtype = !subtype.is_empty() && subtype != "all";
        let file_specific = secondary_template_filename(&mime, false);
        let file_category = secondary_template_filename(&mime, true);

        let scope_row = adw::ComboRow::builder()
            .title(tr("Save to").as_str())
            .subtitle(tr("A type-specific file replaces broader templates for that type").as_str())
            .model(&gtk::StringList::new(&[
                file_specific.as_str(),
                file_category.as_str(),
            ]))
            .build();
        scope_row.set_visible(has_subtype && !category.is_empty());

        let initial_mime = default_mime(&mime, false);
        let last_default = Rc::new(RefCell::new(initial_mime.clone()));

        let (label_row, label_entry) = icon_entry_row(&tr("Label"), "", &dialog);
        let (sub_row, sub_entry) = icon_entry_row(&tr("Submenu (blank = top-level)"), "", &dialog);
        let (mime_row, mime_entry) = entry_row(&tr("MIME Types"), &initial_mime);
        let (cmd_row, cmd_entry) =
            entry_row(&tr("Command (%p = path · %d = dir · %f = filename)"), "");
        let (toast_row, toast_entry) = entry_row(&tr("Notification (optional)"), "");

        let no_transfer_switch = gtk::Switch::builder()
            .valign(gtk::Align::Center)
            .margin_end(12)
            .build();
        let no_transfer_row = adw::ActionRow::builder()
            .title(tr("Suppress Transfer Dialog").as_str())
            .subtitle(tr("Do not track execution progress or open transfer dialog").as_str())
            .activatable_widget(&no_transfer_switch)
            .build();
        no_transfer_row.add_suffix(&no_transfer_switch);

        // Switching scope rewrites the MIME field, unless the user already edited it.
        {
            let mime = mime.clone();
            let mime_entry = mime_entry.clone();
            let last_default = last_default.clone();
            scope_row.connect_selected_notify(move |row| {
                let next = default_mime(&mime, row.selected() == 1);
                if mime_entry.text().as_str() == last_default.borrow().as_str() {
                    mime_entry.set_text(&next);
                }
                *last_default.borrow_mut() = next;
            });
        }

        g_id.add(&scope_row);
        g_id.add(&label_row);
        g_id.add(&sub_row);
        g_act.add(&mime_row);
        g_act.add(&cmd_row);
        g_act.add(&toast_row);
        g_act.add(&no_transfer_row);
        page.add(&g_id);
        page.add(&g_act);
        scroller.set_child(Some(&page));
        outer.append(&scroller);

        let btn_bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .halign(gtk::Align::End)
            .spacing(8)
            .margin_top(12)
            .margin_bottom(20)
            .margin_end(20)
            .build();
        let cancel_btn = gtk::Button::builder().label(tr("Cancel").as_str()).build();
        let add_btn = gtk::Button::builder()
            .label(tr("Add").as_str())
            .css_classes(["suggested-action"])
            .build();
        btn_bar.append(&cancel_btn);
        btn_bar.append(&add_btn);
        outer.append(&btn_bar);
        dialog.set_child(Some(&outer));

        {
            let d = dialog.clone();
            cancel_btn.connect_clicked(move |_| {
                d.close();
            });
        }

        let focus_target = label_entry.clone();
        {
            let d = dialog.clone();
            let s = sender.clone();
            add_btn.connect_clicked(move |_| {
                let mut ok = true;
                for e in [&label_entry, &mime_entry, &cmd_entry] {
                    if e.text().trim().is_empty() {
                        e.add_css_class("error");
                        ok = false;
                    } else {
                        e.remove_css_class("error");
                    }
                }
                if !ok {
                    return;
                }
                let opt = |e: &gtk::Entry| {
                    let v = e.text().trim().to_string();
                    (!v.is_empty()).then_some(v)
                };
                s.input(AppMsg::SaveContextualAction {
                    mime: mime.clone(),
                    category_scope: scope_row.is_visible() && scope_row.selected() == 1,
                    label: label_entry.text().trim().to_string(),
                    submenu: opt(&sub_entry),
                    mime_types: mime_entry.text().trim().to_string(),
                    command: cmd_entry.text().trim().to_string(),
                    toast: opt(&toast_entry),
                    no_command_dialog: no_transfer_switch.is_active(),
                });
                d.close();
            });
        }

        dialog.present(gtk::Application::default().active_window().as_ref());
        focus_target.grab_focus();
    }
}
