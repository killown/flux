use adw::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

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
            .connect_activate(move |_, _| sender_clone.input(AppMsg::Open));

        // 6. Sidebar and Volume Monitoring
        let listbox = gtk::ListBox::default();
        let sidebar =
            FactoryVecDeque::builder()
                .launch(listbox)
                .forward(sender.input_sender(), |msg| match msg {
                    crate::ui::SidebarMsg::Navigate(path) => AppMsg::Navigate(path),
                    crate::ui::SidebarMsg::Remove(path) => AppMsg::RemoveFromSidebar(path),
                });

        let volume_monitor = gio::VolumeMonitor::get();
        let s_added = sender.clone();
        volume_monitor.connect_mount_added(move |_, _| s_added.input(AppMsg::RefreshSidebar));

        let s_removed = sender.clone();
        volume_monitor.connect_mount_removed(move |_, _| s_removed.input(AppMsg::RefreshSidebar));

        // 7. Breadcrumb Setup (Returned for local_ref)
        let breadcrumb_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let breadcrumbs = FactoryVecDeque::builder()
            .launch(breadcrumb_box.clone())
            .forward(sender.input_sender(), AppMsg::Navigate);

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
            task_progress: None,
            toast_overlay: adw::ToastOverlay::new(),
            pending_toasts: std::collections::HashMap::new(),
        };

        // 9. Initial State Population
        model.setup_actions(&sender);
        model.recent_stack.push_front(start_path.clone());
        model.update_breadcrumbs();

        for place in &config.sidebar {
            model.sidebar.guard().push_back(SidebarPlace {
                name: place.name.clone(),
                icon: place.icon.clone(),
                path: utils::expand_path(&place.path),
            });
        }
        model.refresh_sidebar();

        // 10. Global Action Registration
        let app = relm4::main_adw_application();
        let sender_reload = sender.clone();
        let reload_action = gio::SimpleAction::new("reload-sidebar", None);
        reload_action.connect_activate(move |_, _| {
            sender_reload.input(AppMsg::RefreshSidebar);
        });
        app.add_action(&reload_action);

        // Queue initial data fetch
        let s_init = sender.clone();
        glib::idle_add_local_once(move || {
            s_init.input(AppMsg::Refresh);
        });

        (model, breadcrumb_box)
    }
}
