//! User Interface module for the Flux file manager.
//!
//! This module groups all GTK/Relm4 widget definitions and view-related
//! logic to separate the presentation layer from the core application state.

// --- 1. Submodule Declarations ---
pub mod init;
pub mod inputs;
pub mod keymap;
pub mod location_dialog;
pub mod settings;
pub mod update;
pub mod view;
pub use settings::SettingsWindow;
pub mod menu_editor;
pub use components::SidebarMsg;
pub mod app_action_ops;
pub mod command_dialog;
pub mod config_handlers;
pub mod context_menu;
pub mod dialogs;
pub mod file_ops;
pub mod navigation;
pub mod network_dialogs;
pub mod paste_ops;
pub mod remote_ops;
pub mod search_handlers;
pub mod sidebar_network;
pub mod sidebar_ops;
pub mod task_ops;
pub mod terminal_ops;
pub mod transfer_dialog;
pub mod view_ops;
pub mod watcher_ops;

// --- 2. Existing Submodules ---
mod components;
mod help;
mod properties;

// --- 3. Public Re-exports ---
// This allows main.rs to call `ui::FluxApp`
pub use components::{FileItem, SidebarPlace};
pub use help::HelpWindow;
pub use properties::FileProperties;

/// Groups UI-specific constants such as default window dimensions or CSS class names.
pub mod constants {
    pub const DEFAULT_WIDTH: i32 = 1100;
    pub const DEFAULT_HEIGHT: i32 = 750;

    // CSS Classes
    pub const CARD_CSS_CLASS: &str = "flux-card";
    pub const SIDEBAR_CSS_CLASS: &str = "sidebar";
    pub const BREADCRUMB_BTN_CLASS: &str = "breadcrumb-btn";
    pub const SIDEBAR_ROW_CLASS: &str = "sidebar-row";
    pub const SIDEBAR_LABEL_CLASS: &str = "sidebar-label";
    pub const SIDEBAR_SECTION_ROW_CLASS: &str = "sidebar-section-row";
    pub const SIDEBAR_SECTION_LABEL_CLASS: &str = "sidebar-section-label";
    pub const THUMBNAIL_CLASS: &str = "thumbnail";
    pub const FLUX_LABEL_CLASS: &str = "flux-label";
    pub const RENAME_ENTRY_CLASS: &str = "flux-rename-entry";
    pub const DESTRUCTIVE_ACTION_CLASS: &str = "destructive-action";
    pub const SORT_CONTAINER_CLASS: &str = "sort-container";
    pub const SORT_LABEL_CLASS: &str = "sort-status-label";

    // View Names (Stack/Navigation children)
    pub const VIEW_PATH: &str = "path";
    pub const VIEW_ENTRY: &str = "entry";
    pub const VIEW_SEARCH: &str = "search";
    pub const VIEW_LABEL: &str = "label";
    pub const VIEW_FILTER: &str = "filter";
    pub const ICON_FILTER: &str = "view-filter-symbolic";
    pub const FILTER_BAR_CSS_CLASS: &str = "flux-filter-bar";

    // Thumbnails
    pub const CACHED_THUMBNAIL_SIZE: i32 = 512;

    // Layout Constraints
    pub const SCROLLED_WINDOW_MIN_WIDTH: i32 = 480;
    pub const LOCATION_ENTRY_WIDTH_REQUEST: i32 = 450;
    pub const SEARCH_ENTRY_WIDTH_REQUEST: i32 = 450;
    pub const RECENT_STACK_CAPACITY: usize = 10;
    pub const MAX_RECENT_ITEMS: usize = 9;
    pub const SIDEBAR_SPACING: i32 = 18;
    pub const HEADER_BTN_SPACING: i32 = 6;
    pub const STATUS_ICON_SPACING: i32 = 8;
    pub const HEADER_MARGIN_END: i32 = 12;
    pub const MAX_BREADCRUMBS: usize = 5;

    // Widget Specifics
    pub const STATUS_ICON_SIZE: i32 = 16;

    // Visual Polish
    pub const OPACITY_ICON: f64 = 0.6;
    pub const OPACITY_LABEL: f64 = 0.8;

    // Zoom Limits
    pub const ZOOM_STEP: i32 = 32;
    pub const ZOOM_MIN: i32 = 16;
    pub const ZOOM_MAX: i32 = 480;

    // MIME Types
    pub const MIME_DIR: &str = "inode/directory";
    pub const MIME_TEXT: &str = "text/plain";
    pub const MIME_EMPTY: &str = "inode/x-empty";

    // Action Filtering Keywords
    pub const FILTER_ALL: &str = "*";
    pub const FILTER_TRASH: &str = "trash";
    pub const FILTER_FOLDER: &str = "folder";
    pub const FILTER_FILE: &str = "file";

    // Shell & Command Templates
    pub const TEMPLATE_PATHS: &str = "%p";
    pub const TEMPLATE_CWD: &str = "%d";

    // Icons & URIs
    pub const ICON_BACK: &str = "go-previous-symbolic";
    pub const ICON_FORWARD: &str = "go-next-symbolic";
    pub const ICON_TRASH: &str = "user-trash-full-symbolic";
    pub const ICON_SORT_INDICATOR: &str = "view-sort-ascending-symbolic";
    pub const TRASH_URI: &str = "trash:///";
    pub const RECENT_URI: &str = "recent:///";

    // Mouse Buttons
    pub const MOUSE_BACK: u32 = 8;
    pub const MOUSE_FORWARD: u32 = 9;
    pub const MOUSE_RIGHT_CLICK: u32 = 3;
    pub const MOUSE_MIDDLE: u32 = 2;

    // Text Labels & Tooltips
    pub const BREADCRUMB_MAX_WIDTH_CHARS: u32 = 20;
    pub const SWIPE_VELOCITY_THRESHOLD: f64 = 500.0;
}
