use adw::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::ui::{constants, FileItem, SidebarPlace};
use crate::utils;
use gtk::gio;
use gtk::glib;

impl FluxApp {
    /// Performs the imperative setup for the Flux application.
    ///
    /// This method decouples the complex state initialization and system monitoring
    /// from the main component file to improve maintainability and compilation speed.
    pub(crate) fn init_components(
        start_path: PathBuf,
        root: &adw::Window,
        sender: AsyncComponentSender<Self>,
    ) -> (Self, gtk::Box) {
        // 1. Static and Global Configuration
        let _ = crate::model::SENDER.set(sender.input_sender().clone());
        relm4::set_global_css(include_str!("style.css"));

        // 2. Resource Loading
        let config = utils::load_config();
        let menu_actions_list = utils::load_menu_config();
        let context_menu_popover = gtk::PopoverMenu::builder().has_arrow(false).build();
        let state_db = Arc::new(crate::services::db::StateManager::new().expect("DB Init Failed"));

        // 3. Background Database Maintenance
        let scrub_db = state_db.clone();
        std::thread::spawn(move || {
            if let Err(e) = scrub_db.scrub_orphans() {
                eprintln!("[DB] Scrub failed: {}", e);
            }
        });

        // 4. Action and Input Controllers
        let shortcut_controller = gtk::ShortcutController::new();
        Self::setup_shortcuts(&shortcut_controller, &sender);
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

        // 5. Grid View Configuration
        let files = TypedGridView::<FileItem, gtk::MultiSelection>::new();
        files.view.set_enable_rubberband(true);
        files.view.set_single_click_activate(config.ui.single_click);
        let sender_selection = sender.clone();
        if let Some(selection_model) = files.view.model().and_downcast::<gtk::MultiSelection>() {
            selection_model.connect_selection_changed(move |_, _, _| {
                sender_selection.input(AppMsg::SelectionChanged);
            });
        }
        files.view.set_max_columns(20);
        files.view.set_min_columns(1);

        let sender_clone = sender.clone();
        files
            .view
            .connect_activate(move |_, position| sender_clone.input(AppMsg::Open(Some(position))));

        // 6. Sidebar and Volume Monitoring
        let listbox = gtk::ListBox::default();
        let sidebar = FactoryVecDeque::builder().launch(listbox.clone()).forward(
            sender.input_sender(),
            |msg| match msg {
                crate::ui::SidebarMsg::Navigate(path) => AppMsg::Navigate(path),
                crate::ui::SidebarMsg::Remove(path) => AppMsg::RemoveFromSidebar(path),
                crate::ui::SidebarMsg::Reorder { from, to } => AppMsg::ReorderSidebar { from, to },
            },
        );

        let volume_monitor = gio::VolumeMonitor::get();
        crate::ui::sidebar_network::connect_volume_monitor_signals(sender.input_sender().clone());

        let network_section = crate::ui::sidebar_network::build_network_section(
            &config.network_bookmarks,
            sender.input_sender().clone(),
        );

        let sidebar_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar_container.append(&listbox);
        sidebar_container.append(&network_section);

        let sidebar_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar_container.append(sidebar.widget());
        sidebar_container.append(&network_section);

        // 7. Breadcrumb Setup (Returned for local_ref)
        let breadcrumb_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let breadcrumbs = FactoryVecDeque::builder()
            .launch(breadcrumb_box.clone())
            .forward(sender.input_sender(), AppMsg::Navigate);

        // Terminal setup using custom terminal implementation from services
        let terminal = crate::services::terminal::Terminal::new(&config.ui.terminal);
        terminal.apply_theme(&config.ui.terminal);
        terminal.connect_theme_changes(config.ui.terminal.clone());

        {
            let s = sender.clone();
            terminal.set_cwd_callback(move |path| {
                s.input(AppMsg::Navigate(path));
            });
        }

        // Intercept F4 and clipboard shortcuts before terminal consumes them.
        let term_sender = sender.clone();
        let term_key_ctrl = gtk::EventControllerKey::new();
        term_key_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let terminal_ref = terminal.clone();
            term_key_ctrl.connect_key_pressed(move |_, keyval, _, modifiers| {
                use gtk::gdk::Key;

                let ctrl_shift =
                    gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK;

                match keyval {
                    Key::F4 => {
                        term_sender.input(AppMsg::ToggleTerminal);
                        return glib::Propagation::Stop;
                    }
                    // Ctrl+Shift+C → copy selection to clipboard
                    Key::c | Key::C if modifiers == ctrl_shift => {
                        terminal_ref.emit_copy_clipboard();
                        return glib::Propagation::Stop;
                    }
                    // Ctrl+Shift+V → paste from clipboard into terminal
                    Key::v | Key::V if modifiers == ctrl_shift => {
                        terminal_ref.emit_paste_clipboard();
                        return glib::Propagation::Stop;
                    }
                    // Feed Tab directly to the PTY so shell completion works.
                    Key::Tab if modifiers.is_empty() => {
                        terminal_ref.feed_child(b"\t");
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }

                glib::Propagation::Proceed
            });
        }
        terminal.add_controller(&term_key_ctrl);

        // Middle-click paste from the primary selection.
        {
            // Suppress right-click menu.
            let right_click = gtk::GestureClick::new();
            right_click.set_button(3);
            right_click.set_propagation_phase(gtk::PropagationPhase::Capture);
            right_click.connect_pressed(|gesture, _, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
            });
            terminal.add_controller(&right_click);

            let terminal_ref = terminal.clone();
            let middle_click = gtk::GestureClick::new();
            middle_click.set_button(2); // BUTTON_MIDDLE
            middle_click.set_propagation_phase(gtk::PropagationPhase::Capture);
            middle_click.connect_pressed(move |gesture, _, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                terminal_ref.emit_paste_clipboard();
            });
            terminal.add_controller(&middle_click);
        }

        let term_for_shutdown = terminal.clone();
        root.connect_close_request(move |_| {
            term_for_shutdown.kill_shell();
            glib::Propagation::Proceed
        });

        let app = relm4::main_adw_application();
        let term_for_app_shutdown = terminal.clone();
        app.connect_shutdown(move |_| {
            term_for_app_shutdown.kill_shell();
        });

        // 8. Model Assembly
        let mut model = FluxApp {
            files,
            sidebar,
            breadcrumbs,
            current_path: start_path.clone(),
            history: Vec::new(),
            forward_stack: Vec::new(),
            load_id: Arc::new(AtomicU64::new(0)),
            current_icon_size: config.ui.default_icon_size,
            context_menu_popover,
            menu_actions: menu_actions_list,
            active_item_path: None,
            directory_monitor: None,
            action_group,
            exclusive_list: Vec::new(),
            exclusive_index: None,
            search_just_opened: false,
            sort_by: config.ui.default_sort,
            sort_ascending: true,
            show_hidden: config.ui.show_hidden_by_default,
            keymap: crate::ui::keymap::KeyMap::new(&config.shortcuts),
            config: config.clone(),
            _volume_monitor: volume_monitor,
            filter: String::new(),
            header_view: constants::VIEW_PATH.to_string(),
            recent_stack: std::collections::VecDeque::with_capacity(
                constants::RECENT_STACK_CAPACITY,
            ),
            state_db,
            selection_status: String::new(),
            is_loading: false,
            task_queue: crate::services::tasks::new_queue(),
            toast_overlay: adw::ToastOverlay::new(),
            pending_toasts: std::collections::HashMap::new(),
            terminal,
            terminal_visible: false,
            terminal_spawned: false,
            terminal_cleared: false,
            terminal_paned: None,
            sidebar_visible: config.ui.sidebar_visible,
            sidebar_widget: Some(sidebar_container.upcast()),
            recents_has_selection: false,
            recents_label: tr("Clear Recents"),
            recents_tooltip: tr("Clear all recents"),
            quick_panel_box: gtk::Box::new(gtk::Orientation::Horizontal, 0),
            cached_archive_password: None,
            archive_locked: false,
            network_section,
        };

        // 9. Initial State Population
        model.setup_actions(&sender);
        model.recent_stack.push_front(start_path.clone());
        model.update_breadcrumbs();

        for place in &config.sidebar {
            model.sidebar.guard().push_back(SidebarPlace {
                name: place.name.clone(),
                icon: place.icon.clone(),
                path: if place.kind.as_deref() == Some("label") {
                    std::path::PathBuf::new()
                } else {
                    utils::expand_path(&place.path)
                },
                is_mount: false,
                is_section_label: place.kind.as_deref() == Some("label"),
            });
        }
        model.refresh_sidebar();

        // Throttled task queue UI refresh, deferred by 150ms, updates status bar only.
        let tick_sender = sender.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            let sender_inner = tick_sender.clone();
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                sender_inner.input(AppMsg::TaskQueueTick);
                glib::ControlFlow::Continue
            });
            glib::ControlFlow::Break
        });

        // 10. Global Action Registration
        let app = relm4::main_adw_application();
        let sender_reload = sender.clone();
        let reload_action = gio::SimpleAction::new("reload-sidebar", None);
        reload_action.connect_activate(move |_, _| {
            sender_reload.input(AppMsg::RefreshSidebar);
        });
        app.add_action(&reload_action);

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

        // Queue initial data fetch
        let s_init = sender.clone();
        glib::idle_add_local_once(move || {
            s_init.input(AppMsg::Refresh);
        });

        (model, breadcrumb_box)
    }

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
