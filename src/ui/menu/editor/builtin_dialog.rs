use super::builtins::BUILTINS;
use crate::i18n::tr;
use adw::gdk;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use std::rc::Rc;

pub(super) fn show_builtin_dialog(
    parent: Option<&gtk::Window>,
    on_select: impl Fn(&str, &str) + 'static,
) {
    let dialog = adw::Window::builder()
        .title(tr("Select Built-in Action").as_str())
        .modal(true)
        .default_width(620)
        .default_height(540)
        .resizable(false)
        .build();

    if let Some(win) = parent {
        dialog.set_transient_for(Some(win));
    }

    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    let header = adw::HeaderBar::builder()
        .show_start_title_buttons(false)
        .show_end_title_buttons(true)
        .build();
    root.append(&header);

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(tr("Search built-ins…").as_str())
        .margin_start(16)
        .margin_end(16)
        .margin_top(8)
        .margin_bottom(8)
        .build();
    root.append(&search_entry);

    let list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .margin_start(16)
        .margin_end(16)
        .margin_bottom(16)
        .build();

    let on_select = Rc::new(on_select);

    for b in BUILTINS {
        let desc_translated = tr(b.desc);
        let scope_label = tr("Scope");
        let row = adw::ActionRow::builder()
            .title(b.name)
            .subtitle(format!("{}: {} │ {}", scope_label, b.scope, desc_translated).as_str())
            .build();

        let copy_btn = gtk::Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text(tr("Copy to clipboard").as_str())
            .css_classes(["flat", "circular"])
            .valign(gtk::Align::Center)
            .build();

        let name_to_copy = b.name;
        copy_btn.connect_clicked(move |_| {
            if let Some(display) = gdk::Display::default() {
                display.clipboard().set_text(name_to_copy);
            }
        });

        let use_btn = gtk::Button::builder()
            .label(tr("Use").as_str())
            .css_classes(["suggested-action", "pill"])
            .valign(gtk::Align::Center)
            .build();

        let d_clone = dialog.clone();
        let cb = on_select.clone();
        let cmd_name = b.name;
        let scope_name = b.scope;
        use_btn.connect_clicked(move |_| {
            cb(cmd_name, scope_name);
            d_clone.close();
        });

        let btn_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .valign(gtk::Align::Center)
            .build();
        btn_box.append(&copy_btn);
        btn_box.append(&use_btn);

        row.add_suffix(&btn_box);

        let search_meta = format!("{} {} {}", b.name, b.scope, b.desc).to_lowercase();
        unsafe {
            row.set_data("search-meta", search_meta);
        }

        list_box.append(&row);
    }

    let list_filter = list_box.clone();
    search_entry.connect_search_changed(move |entry| {
        let query = entry.text().trim().to_lowercase();
        let mut child = list_filter.first_child();
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

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&list_box)
        .build();
    root.append(&scroller);

    let d_esc = dialog.clone();
    let esc = gtk::ShortcutController::new();
    esc.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("Escape"),
        Some(gtk::CallbackAction::new(move |_, _| {
            d_esc.close();
            glib::Propagation::Stop
        })),
    ));
    dialog.add_controller(esc);

    dialog.set_content(Some(&root));
    dialog.present();
}
