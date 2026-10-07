use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;

impl FluxApp {
    /// Installs shortcuts and the `win` action group with the sidebar toggle.
    pub(super) fn register_window_actions(
        root: &adw::Window,
        sender: &AsyncComponentSender<Self>,
    ) -> gio::SimpleActionGroup {
        let shortcut_controller = gtk::ShortcutController::new();
        Self::setup_shortcuts(&shortcut_controller, sender);
        root.add_controller(shortcut_controller);

        let action_group = gio::SimpleActionGroup::new();
        root.insert_action_group("win", Some(&action_group));

        // Register sidebar toggle action BEFORE moving action_group into model
        let toggle_sidebar_action = gio::SimpleAction::new("toggle-sidebar", None);
        let s_sidebar = sender.clone();
        toggle_sidebar_action.connect_activate(move |_, _| {
            s_sidebar.input(AppMsg::ToggleSidebar);
        });
        action_group.add_action(&toggle_sidebar_action);
        action_group
    }

    /// Registers app-level actions and icon theme change listeners.
    pub(super) fn register_app_actions(
        model: &FluxApp,
        root: &adw::Window,
        sender: &AsyncComponentSender<Self>,
    ) {
        let app = relm4::main_adw_application();
        let sender_reload = sender.clone();
        let reload_action = gio::SimpleAction::new("reload-sidebar", None);
        reload_action.connect_activate(move |_, _| {
            sender_reload.input(AppMsg::RefreshSidebar);
        });
        app.add_action(&reload_action);

        // ── Register Connect to Server action ──
        crate::ui::dialog::network::register_connect_action(
            &app,
            root,
            sender.input_sender().clone(),
        );

        // Stateful radio action: GIO compares each menu item's target against this
        // action's state and renders a native radio checkmark on the matching item.
        let sort_field_action = gio::SimpleAction::new_stateful(
            "sort-field",
            Some(&String::static_variant_type()),
            &model.sort_by.as_action_state(),
        );
        {
            let s = sender.clone();
            sort_field_action.connect_activate(move |action, target| {
                if let Some(v) = target {
                    action.set_state(v);
                    if let Some(key) = v.str() {
                        s.input(AppMsg::SetDefaultSort(
                            crate::model::SortBy::from_action_key(key),
                        ));
                    }
                }
            });
        }
        app.add_action(&sort_field_action);

        let sort_dir_action = gio::SimpleAction::new_stateful(
            "sort-direction",
            Some(&String::static_variant_type()),
            &model.sort_direction_state(),
        );
        {
            let s = sender.clone();
            sort_dir_action.connect_activate(move |action, target| {
                if let Some(v) = target {
                    action.set_state(v);
                    if let Some(key) = v.str() {
                        s.input(AppMsg::SetAsc(key == "asc"));
                    }
                }
            });
        }
        app.add_action(&sort_dir_action);

        let s_about = sender.clone();
        let action_about = gio::SimpleAction::new("show-about", None);
        action_about.connect_activate(move |_, _| {
            s_about.input(AppMsg::ShowAbout);
        });
        app.add_action(&action_about);
        app.set_accels_for_action("app.show-about", &[]);

        // Icon Theme Change Listeners
        if let Some(display) = gtk::gdk::Display::default() {
            let icon_theme = gtk::IconTheme::for_display(&display);
            let s_icon = sender.clone();
            icon_theme.connect_changed(move |_| {
                crate::utils::icon::invalidate_themed_icon_cache();
                crate::services::loader::invalidate_extension_icon_cache();
                s_icon.input(AppMsg::Refresh);
            });
        }

        if let Some(settings) = gtk::Settings::default() {
            let s_settings = sender.clone();
            settings.connect_gtk_icon_theme_name_notify(move |_| {
                crate::utils::icon::invalidate_themed_icon_cache();
                crate::services::loader::invalidate_extension_icon_cache();
                s_settings.input(AppMsg::Refresh);
            });
        }
    }
}
