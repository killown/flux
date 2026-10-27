use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

impl FluxApp {
    /// Performs the imperative setup for the Flux application.
    ///
    /// This method decouples the complex state initialization and system monitoring
    /// from the main component file to improve maintainability and compilation speed.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn init_components(
        start_path: PathBuf,
        quick_list: Option<Vec<PathBuf>>,
        initial_tag_search: Option<String>,
        no_sidebar: bool,
        no_header: bool,
        no_statusbar: bool,
        root: &adw::Window,
        sender: AsyncComponentSender<Self>,
    ) -> (Self, gtk::Box) {
        crate::hit!("init_components");

        // Static and Global Configuration
        let _ = crate::model::SENDER.set(sender.input_sender().clone());
        {
            crate::hit!("init_components:css");
            relm4::set_global_css(include_str!("../style.css"));
        }

        // Resource Loading (config and menu awaited, the DB opens in the background)
        let (pending_db, config, menu_actions_list) = Self::load_resources().await;

        let context_menu_popover = gtk::PopoverMenu::builder().has_arrow(false).build();

        crate::utils::helpers::apply_ui_scale(config.ui.ui_scale);

        // Action and Input Controllers
        let action_group = Self::register_window_actions(root, &sender);

        // Grid View Configuration
        let files = {
            crate::hit!("init_components:grid");
            Self::build_file_grid(&config, &sender)
        };

        // Tab Container Initialization
        let (tab_view, tab_bar, initial_tab) = {
            crate::hit!("init_components:tabs");
            Self::build_tabs(start_path.clone(), &config, &sender)
        };

        // Sidebar and Volume Monitoring
        let (sidebar, volume_monitor, network_section, sidebar_root) = {
            crate::hit!("init_components:sidebar");
            Self::build_sidebar(&config, &sender)
        };

        // Breadcrumb Setup (Returned for local_ref)
        let breadcrumb_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let breadcrumbs = FactoryVecDeque::builder()
            .launch(breadcrumb_box.clone())
            .forward(sender.input_sender(), AppMsg::Navigate);

        // Terminal setup using custom terminal implementation from services
        let terminal = Self::build_terminal(&config, root, &sender);

        // The DB has been opening in parallel with the UI construction above.
        let state_db = Arc::new(pending_db.wait().await);

        // The saved lists always resume, in the same order, on the same list and
        // the same slot. A list given on the command line is appended as a new
        // list under a unique name and selected for this session, so the lists
        // the user already has are kept. The CLI list is persisted like any
        // other list, so it survives a restart and can be renamed or deleted
        // from the Quick List panel.
        let default_list_name = tr("Quick List");
        let mut initial_quick_groups = state_db.load_quick_groups(&default_list_name);
        if let Some(list) = quick_list {
            let mut deduped: Vec<PathBuf> = Vec::new();
            for p in list {
                if !deduped.contains(&p) {
                    deduped.push(p);
                }
            }
            if !deduped.is_empty() {
                let name = initial_quick_groups.unique_name(&default_list_name);
                if let Ok(idx) = initial_quick_groups.add_list(&name) {
                    initial_quick_groups.add_to(idx, deduped);
                    initial_quick_groups.select(idx);
                }
            }
        }
        let initial_exclusive_list = initial_quick_groups.selected_list().items.clone();
        let initial_exclusive_index = initial_quick_groups.selected_list().index;

        // Model Assembly
        let initial_icon_size = dirs::home_dir()
            .and_then(|home| state_db.get_view(&home).ok().flatten())
            .map(|(_, _, size, _)| size as i32)
            .unwrap_or(config.ui.default_icon_size);

        let effective_sidebar_visible = if no_sidebar {
            false
        } else {
            config.ui.sidebar_visible
        };
        let effective_header_visible = if no_header {
            false
        } else {
            config.ui.header_visible
        };

        let mut model = FluxApp {
            startup_preview: false,
            mime_filter: None,
            mime_filter_from_space: false,
            preview_cache: std::sync::Arc::new(
                crate::app::update::view::prefetch::PreviewCache::default(),
            ),
            preview_view_stack: None,
            preview_view_switcher: None,
            preview_panel_revealer: None,
            preview_panel_visible: false,
            preview_panel_initialized: false,
            wide_preview_sidebar: None,
            preview_fullscreen: None,
            preview_session: Arc::new(AtomicU64::new(0)),
            connect_panel_revealer: None,
            connect_panel_visible: false,
            connect_panel_initialized: false,
            location_panel_revealer: None,
            location_panel_visible: false,
            location_panel_initialized: false,
            location_entry: None,
            diff_panel_revealer: None,
            diff_panel_visible: false,
            diff_panel_initialized: false,
            diff_text_buffer: None,
            active_diff_target: None,
            git_status_dir: PathBuf::new(),
            git_status_map: std::collections::HashMap::new(),
            is_in_git_repo: false,
            tab_view,
            tab_bar,
            tabs: vec![initial_tab],
            active_tab_index: 0,
            next_tab_id: 1,
            tag_panel_revealer: None,
            tag_panel_visible: false,
            tag_panel_initialized: false,
            search_panel_revealer: None,
            search_panel_visible: false,
            search_panel_initialized: false,
            actions_panel_revealer: None,
            actions_panel_visible: false,
            actions_panel_initialized: false,
            actions_panel_session: Arc::new(AtomicU64::new(0)),
            properties_panel_revealer: None,
            properties_panel_visible: false,
            properties_panel_initialized: false,
            properties_panel_session: Arc::new(AtomicU64::new(0)),
            properties_target: None,
            quick_list_panel_revealer: None,
            quick_list_panel_visible: false,
            quick_list_sidebar_stashed: None,
            quick_list_panel_initialized: false,
            active_video_preview: None,
            video_preview_source: None,
            files,
            sidebar,
            breadcrumbs,
            current_path: start_path.clone(),
            current_icon_size: initial_icon_size,
            alt_cycle: None,
            navigate: None,
            history: Vec::new(),
            forward_stack: Vec::new(),
            load_id: Arc::new(AtomicU64::new(0)),
            current_list_icon_size: config.ui.list_icon_size,
            context_menu_popover,
            menu_actions: menu_actions_list,
            active_item_path: None,
            directory_monitor: None,
            action_group,
            exclusive_list: initial_exclusive_list,
            exclusive_index: initial_exclusive_index,
            quick_groups: initial_quick_groups,
            extension_globset: None,
            search_just_opened: false,
            sort_by: config.ui.default_sort,
            sort_ascending: true,
            show_hidden: config.ui.show_hidden_by_default,
            keymap: crate::ui::keymap::KeyMap::new(&config.shortcuts),
            config: config.clone(),
            _volume_monitor: volume_monitor,
            filter: String::new(),
            extension_filter: None,
            filter_mode: crate::model::FilterMode::All,
            is_list_mode: config.default_list_mode,
            header_view: constants::VIEW_PATH.to_string(),
            recent_stack: std::collections::VecDeque::with_capacity(
                constants::RECENT_STACK_CAPACITY,
            ),
            state_db: state_db.clone(),
            selection_status: String::new(),
            is_loading: false,
            task_queue: crate::infra::tasks::new_queue(),
            toast_overlay: adw::ToastOverlay::new(),
            last_toast: None,
            terminal,
            terminal_visible: false,
            terminal_fullscreen: false,
            main_stack: None,
            root_window: Some(root.clone()),
            fullscreen_terminal: None,
            fullscreen_terminal_box: None,
            terminal_spawned_fullscreen: false,
            terminal_spawned: false,
            terminal_cleared: false,
            terminal_panel_revealer: None,
            terminal_panel_visible: false,
            terminal_panel_initialized: false,
            terminal_sidebar_stashed: None,
            sidebar_terminal: None,
            terminal_paned: None,
            sidebar_visible: effective_sidebar_visible,
            sidebar_widget: Some(sidebar_root.upcast()),
            header_visible: effective_header_visible,
            header_widget: None,
            statusbar_visible: !no_statusbar,
            recents_has_selection: false,
            recents_label: tr("Clear Recents"),
            recents_tooltip: tr("Clear all recents"),
            cached_archive_passwords: std::collections::HashMap::new(),
            known_encrypted_archives: std::collections::HashSet::new(),
            pending_keyring_saves: crate::infra::keyring::PendingSaves::default(),
            archive_locked: false,
            network_section,
            is_content_searching: false,
            content_search_cancellable: None,
            saved_list_mode: false,
            saved_max_columns: 20,
            search_saved_layout: false,
            transfer_dialog: None,
            command_dialog: None,
            header_path_entry: glib::WeakRef::new(),
            conflict_dialog_active: false,
            file_op_history: crate::ui::undo_redo::FileOpHistory::new(),
            pending_thumbnails: std::collections::HashSet::new(),
            folder_cache: std::collections::HashMap::with_capacity(32),
            scrolled_to_bottom: false,
            last_search_was_advanced: false,
            active_item_line: 0,
            thumbnail_manager: Arc::new(crate::infra::thumbnails::ThumbnailTaskManager::default()),
            video_preview_launches: std::collections::VecDeque::new(),
            video_preview_cooldown_until: None,
        };

        // Apply initial list/grid mode to the view
        if model.is_list_mode {
            model.files.view.set_max_columns(1);
            model.files.view.set_min_columns(1);
        } else {
            model.files.view.set_max_columns(20);
            model.files.view.set_min_columns(1);
        }
        model.saved_list_mode = model.is_list_mode;

        // Initial State Population
        {
            crate::hit!("init_components:actions");
            model.setup_actions(&sender);
        }

        model.recent_stack.push_front(start_path.clone());
        model.update_breadcrumbs();

        Self::start_task_queue_tick(&sender);

        // Global Action Registration + 9.5. Icon Theme Change Listeners
        Self::register_app_actions(&model, &sender);

        // Defer background maintenance and initial data fetching to unblock window presentation
        Self::schedule_deferred_startup(&state_db, initial_tag_search, &sender);

        (model, breadcrumb_box)
    }
}
