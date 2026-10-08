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
        relm4::set_global_css(include_str!("../style.css"));

        // Resource Loading (asynchronous, parallel)
        let (state_db_res, config, menu_actions_list) = Self::load_resources().await;

        let state_db = Arc::new(state_db_res);
        let context_menu_popover = gtk::PopoverMenu::builder().has_arrow(false).build();

        crate::utils::helpers::apply_ui_scale(config.ui.ui_scale);

        // Action and Input Controllers
        let action_group = Self::register_window_actions(root, &sender);

        // Grid View Configuration
        let files = Self::build_file_grid(&config, &sender);

        // Tab Container Initialization
        let (tab_view, tab_bar, initial_tab) =
            Self::build_tabs(start_path.clone(), &config, &sender);

        // Sidebar and Volume Monitoring
        let (sidebar, volume_monitor, network_section, sidebar_root) =
            Self::build_sidebar(&config, &sender);

        // Breadcrumb Setup (Returned for local_ref)
        let breadcrumb_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let breadcrumbs = FactoryVecDeque::builder()
            .launch(breadcrumb_box.clone())
            .forward(sender.input_sender(), AppMsg::Navigate);

        // Terminal setup using custom terminal implementation from services
        let terminal = Self::build_terminal(&config, root, &sender);

        // Fast in-memory deduplication without synchronous disk stats
        let mut initial_exclusive_list = Vec::new();
        let mut initial_exclusive_index = None;
        if let Some(list) = quick_list {
            for p in list {
                if !initial_exclusive_list.contains(&p) {
                    initial_exclusive_list.push(p);
                }
            }
            if !initial_exclusive_list.is_empty() {
                initial_exclusive_index = Some(0);
            }
        }

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
            active_video_preview: None,
            video_preview_source: None,
            files,
            sidebar,
            breadcrumbs,
            current_path: start_path.clone(),
            current_icon_size: initial_icon_size,
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
            is_list_mode: config.default_list_mode,
            header_view: constants::VIEW_PATH.to_string(),
            recent_stack: std::collections::VecDeque::with_capacity(
                constants::RECENT_STACK_CAPACITY,
            ),
            state_db: state_db.clone(),
            selection_status: String::new(),
            is_loading: false,
            task_queue: crate::services::tasks::new_queue(),
            toast_overlay: adw::ToastOverlay::new(),
            last_toast: None,
            terminal,
            terminal_visible: false,
            terminal_spawned: false,
            terminal_cleared: false,
            terminal_paned: None,
            sidebar_visible: effective_sidebar_visible,
            sidebar_widget: Some(sidebar_root.upcast()),
            header_visible: effective_header_visible,
            header_widget: None,
            statusbar_visible: !no_statusbar,
            recents_has_selection: false,
            recents_label: tr("Clear Recents"),
            recents_tooltip: tr("Clear all recents"),
            quick_panel_box: gtk::Box::new(gtk::Orientation::Horizontal, 0),
            cached_archive_password: None,
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
            thumbnail_manager: Arc::new(
                crate::services::thumbnails::ThumbnailTaskManager::default(),
            ),
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

        if !model.exclusive_list.is_empty() {
            model.handle_rebuild_quick_panel(&sender);
        }

        model.recent_stack.push_front(start_path.clone());
        model.update_breadcrumbs();

        Self::start_task_queue_tick(&sender);

        // Global Action Registration + 9.5. Icon Theme Change Listeners
        Self::register_app_actions(&model, root, &sender);

        // Defer background maintenance and initial data fetching to unblock window presentation
        Self::schedule_deferred_startup(&state_db, initial_tag_search, &sender);

        (model, breadcrumb_box)
    }
}
