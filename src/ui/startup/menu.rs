use crate::i18n::tr;
use crate::model::FluxApp;
use adw::prelude::*;
use gtk::glib;

impl FluxApp {
    /// Constructs the `gio::Menu` model for the main hamburger popover.
    ///
    /// Uses GIO menu sections for visual grouping. Each `detailed_action` string
    /// references an `app.*` action registered in `main.rs::setup_shortcuts`.
    pub(crate) fn build_main_menu() -> gtk::gio::Menu {
        let menu = gtk::gio::Menu::new();

        // ── Sort ─────────────────────────────────────────────────────────────
        let sort_submenu = gtk::gio::Menu::new();

        let sort_fields = gtk::gio::Menu::new();
        for (label, key) in [
            (tr("By Name"), "name"),
            (tr("By Date"), "date"),
            (tr("By Size"), "size"),
            (tr("By Type"), "type"),
        ] {
            let item = gtk::gio::MenuItem::new(Some(&label), None);
            item.set_action_and_target_value(
                Some("app.sort-field"),
                Some(&glib::Variant::from(key)),
            );
            sort_fields.append_item(&item);
        }
        sort_submenu.append_section(None, &sort_fields);

        let sort_direction = gtk::gio::Menu::new();
        for (label, key) in [(tr("Ascending"), "asc"), (tr("Descending"), "desc")] {
            let item = gtk::gio::MenuItem::new(Some(&label), None);
            item.set_action_and_target_value(
                Some("app.sort-direction"),
                Some(&glib::Variant::from(key)),
            );
            sort_direction.append_item(&item);
        }
        sort_submenu.append_section(None, &sort_direction);

        menu.append_submenu(Some(&tr("Sort By")), &sort_submenu);

        // ── View & Preferences ───────────────────────────────────────────────
        let view_section = gtk::gio::Menu::new();
        view_section.append(Some(&tr("Preferences")), Some("app.open-settings"));
        view_section.append(Some(&tr("Keyboard Shortcuts")), Some("app.open-help"));

        // Toggle Sidebar - create item manually to set accelerator
        let toggle_item =
            gtk::gio::MenuItem::new(Some(&tr("Toggle Sidebar")), Some("win.toggle-sidebar"));
        toggle_item.set_attribute_value("accel", Some(&"F8".to_variant()));
        view_section.append_item(&toggle_item);

        menu.append_section(None, &view_section);

        // ── Tools ────────────────────────────────────────────────────────────
        let tools_section = gtk::gio::Menu::new();
        tools_section.append(
            Some(&tr("Context Menu Editor")),
            Some("app.open-menu-editor"),
        );
        tools_section.append(Some(&tr("New Window")), Some("app.new-window"));
        menu.append_section(None, &tools_section);

        // ── About ─────────────────────────────────────────────────────────────
        let about_section = gtk::gio::Menu::new();
        about_section.append(Some(&tr("About Flux")), Some("app.show-about"));
        menu.append_section(None, &about_section);

        menu
    }
}
