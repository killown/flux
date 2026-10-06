use crate::i18n::tr;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

mod nerd_fonts;
mod object;

use nerd_fonts::load_installed_json_entries;
use object::NerdIconObject;

pub const MENU_ICON_PADDING: &str = "      ";

pub fn show_menu_icon_picker(parent: Option<&gtk::Window>, on_select: impl Fn(&str) + 'static) {
    let dialog = adw::Window::builder()
        .title(tr("Select Nerd Font Icon").as_str())
        .modal(true)
        .default_width(480)
        .default_height(560)
        .resizable(false)
        .build();

    if let Some(win) = parent {
        dialog.set_transient_for(Some(win));
    }

    // ── Model Setup ──────────────────────────────────────────────────────────
    let store = gio::ListStore::new::<NerdIconObject>();
    let parsed_items = load_installed_json_entries();
    store.extend_from_slice(&parsed_items);

    let search_term: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let search_term_filter = search_term.clone();

    let custom_filter = gtk::CustomFilter::new(move |obj| {
        let term = search_term_filter.borrow();
        if term.is_empty() {
            return true;
        }
        if let Some(item) = obj.downcast_ref::<NerdIconObject>() {
            item.name().to_lowercase().contains(term.as_str())
        } else {
            false
        }
    });

    let filter_model = gtk::FilterListModel::new(Some(store), Some(custom_filter.clone()));
    let selection_model = gtk::SingleSelection::new(Some(filter_model));

    // ── Item Factory ─────────────────────────────────────────────────────────
    let factory = gtk::SignalListItemFactory::new();
    let on_select = Rc::new(on_select);
    let dialog_ref = dialog.clone();

    factory.connect_setup(|_, list_item| {
        let item = list_item.downcast_ref::<gtk::ListItem>().unwrap();

        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .valign(gtk::Align::Center)
            .halign(gtk::Align::Center)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(4)
            .margin_end(4)
            .css_classes(["card"])
            .build();

        let glyph_label = gtk::Label::builder()
            .css_classes(["title-1"])
            .halign(gtk::Align::Center)
            .build();

        let name_label = gtk::Label::builder()
            .css_classes(["caption", "dim-label"])
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(10)
            .halign(gtk::Align::Center)
            .build();

        vbox.append(&glyph_label);
        vbox.append(&name_label);
        item.set_child(Some(&vbox));
    });

    factory.connect_bind(|_, list_item| {
        let item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let icon_obj = item.item().and_downcast::<NerdIconObject>().unwrap();
        let vbox = item.child().and_downcast::<gtk::Box>().unwrap();

        let glyph_lbl = vbox.first_child().and_downcast::<gtk::Label>().unwrap();
        let name_lbl = vbox.last_child().and_downcast::<gtk::Label>().unwrap();

        glyph_lbl.set_label(&icon_obj.glyph());
        name_lbl.set_label(&icon_obj.name());
        vbox.set_tooltip_text(Some(&icon_obj.name()));
    });

    // ── Grid View ────────────────────────────────────────────────────────────
    let grid_view = gtk::GridView::builder()
        .model(&selection_model)
        .factory(&factory)
        .max_columns(6)
        .min_columns(6)
        .enable_rubberband(false)
        .build();

    {
        let cb = on_select.clone();
        let d = dialog_ref.clone();
        grid_view.connect_activate(move |view, pos| {
            let model = view.model().and_downcast::<gtk::SingleSelection>().unwrap();
            if let Some(item) = model.item(pos).and_downcast::<NerdIconObject>() {
                cb(&item.glyph());
                d.close();
            }
        });
    }

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&grid_view)
        .vexpand(true)
        .hexpand(true)
        .build();

    // ── Search Bar & Layout ──────────────────────────────────────────────────
    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text(tr("Search icons (copy, folder, file, rust)...").as_str())
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .margin_bottom(8)
        .build();

    {
        let st = search_term.clone();
        let cf = custom_filter.clone();
        search_entry.connect_search_changed(move |entry| {
            let text = entry.text().trim().to_lowercase();
            *st.borrow_mut() = text;
            cf.changed(gtk::FilterChange::Different);
        });
    }

    let header = adw::HeaderBar::builder()
        .show_start_title_buttons(false)
        .show_end_title_buttons(true)
        .build();

    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    root.append(&header);
    root.append(&search_entry);
    root.append(&scrolled);

    dialog.set_content(Some(&root));

    let entry_focus = search_entry.clone();
    dialog.connect_map(move |_| {
        entry_focus.grab_focus();
    });

    dialog.present();
}
