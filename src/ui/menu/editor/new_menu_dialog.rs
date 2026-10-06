use super::{Msg, Shared};
use crate::i18n::tr;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;

pub(super) fn show_new_menu_dialog(shared: &Shared) {
    let dialog = adw::Window::new();
    dialog.set_title(Some(tr("Create Menu").as_str()));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(&shared.root));
    dialog.set_default_size(520, -1);
    dialog.set_resizable(false);

    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    outer.append(&adw::HeaderBar::new());

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::builder()
        .title(tr("Menu File Name").as_str())
        .description(tr("Enter a menu name (e.g. 'menu-application-all.rs' for 'application/all' MIME type)").as_str())
        .build();

    let name_entry = gtk::Entry::builder()
        .placeholder_text("custom")
        .hexpand(true)
        .margin_top(4)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let row = adw::PreferencesRow::builder().build();
    row.set_child(Some(&name_entry));
    group.add(&row);
    page.add(&group);
    outer.append(&page);

    let btn_bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::End)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(20)
        .margin_end(20)
        .build();

    let cancel_btn = gtk::Button::builder().label(tr("Cancel").as_str()).build();
    let create_btn = gtk::Button::builder()
        .label(tr("Create").as_str())
        .css_classes(["suggested-action"])
        .build();

    btn_bar.append(&cancel_btn);
    btn_bar.append(&create_btn);
    outer.append(&btn_bar);

    dialog.set_content(Some(&outer));

    {
        let d = dialog.clone();
        cancel_btn.connect_clicked(move |_| d.close());
    }

    {
        let d = dialog.clone();
        let esc = gtk::ShortcutController::new();
        esc.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(move |_, _| {
                d.close();
                glib::Propagation::Stop
            })),
        ));
        dialog.add_controller(esc);
    }

    {
        let d = dialog.clone();
        let sender = shared.sender.clone();
        create_btn.connect_clicked(move |_| {
            let text = name_entry.text().to_string();
            if text.trim().is_empty() {
                name_entry.add_css_class("error");
                return;
            }
            name_entry.remove_css_class("error");
            sender.input(Msg::CreateNewMenu(text));
            d.close();
        });
    }

    dialog.present();
}
