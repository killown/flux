//! User Interface module for the Flux file manager.
//!
//! This module groups all GTK/Relm4 widget definitions and view-related
//! logic to separate the presentation layer from the core application state.

// 1. Declare the submodules (internal files)
mod components;
mod help;
mod properties;
pub use help::HelpWindow;

// 2. Publicly re-export specific types for convenience.
// This allows you to call `ui::FileItem` instead of `ui::components::FileItem`.
pub use components::{FileItem, SidebarPlace};
pub use properties::FileProperties;

/// Groups UI-specific constants such as default window dimensions or CSS class names.
pub mod constants {
    pub const APP_TITLE: &str = "flux";
    pub const DEFAULT_WIDTH: i32 = 1100;
    pub const DEFAULT_HEIGHT: i32 = 750;

    // CSS Classes
    pub const CARD_CSS_CLASS: &str = "flux-card";
    pub const SIDEBAR_CSS_CLASS: &str = "sidebar";
    pub const BREADCRUMB_BTN_CLASS: &str = "breadcrumb-btn";
    pub const SIDEBAR_ROW_CLASS: &str = "sidebar-row";
    pub const SIDEBAR_LABEL_CLASS: &str = "sidebar-label";
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

    // Thumbnails
    pub const CACHED_THUMBNAIL_SIZE: i32 = 512;

    // Layout Constraints
    pub const SCROLLED_WINDOW_MIN_WIDTH: i32 = 480;
    pub const LOCATION_ENTRY_WIDTH_REQUEST: i32 = 450;
    pub const SEARCH_ENTRY_WIDTH_REQUEST: i32 = 450;
    pub const RECENT_STACK_CAPACITY: usize = 10;
    pub const MAX_RECENT_ITEMS: usize = 9; // Truncation limit for navigation
    pub const GRID_SPACING: u32 = 16;
    pub const SIDEBAR_SPACING: i32 = 18;
    pub const HEADER_BTN_SPACING: i32 = 6;
    pub const STATUS_ICON_SPACING: i32 = 8;
    pub const HEADER_MARGIN_END: i32 = 12;
    pub const MAX_BREADCRUMBS: usize = 5;

    // Widget Specifics
    pub const MAX_LABEL_CHARS: i32 = 14;
    pub const STATUS_ICON_SIZE: i32 = 16;

    // Visual Polish
    pub const OPACITY_ICON: f64 = 0.6;
    pub const OPACITY_LABEL: f64 = 0.8;

    // Zoom Limits
    pub const ZOOM_STEP: i32 = 32;
    pub const ZOOM_MIN: i32 = 160;
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
    pub const SHELL_BIN: &str = "sh";
    pub const TEMPLATE_PATHS: &str = "%p";
    pub const TEMPLATE_CWD: &str = "%d";

    // Icons & URIs
    pub const ICON_BACK: &str = "go-previous-symbolic";
    pub const ICON_FORWARD: &str = "go-next-symbolic";
    pub const ICON_TRASH: &str = "user-trash-full-symbolic";
    pub const ICON_SORT_INDICATOR: &str = "view-sort-ascending-symbolic";
    pub const TRASH_URI: &str = "trash:///";

    // Mouse Buttons
    pub const MOUSE_BACK: u32 = 8;
    pub const MOUSE_FORWARD: u32 = 9;
    pub const MOUSE_RIGHT_CLICK: u32 = 3;

    // Gesture Thresholds

    /// Minimum velocity (pixels/sec) to trigger a back/forward swipe.
    pub const SWIPE_VELOCITY_THRESHOLD: f64 = 500.0;

    // Text Labels & Tooltips
    pub const LABEL_EMPTY_TRASH: &str = "Empty Trash";
    pub const BREADCRUMB_MAX_WIDTH_CHARS: u32 = 20;
}
