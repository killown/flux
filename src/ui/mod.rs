//! User Interface module for the Flux file manager.
//!
//! This module groups all GTK/Relm4 widget definitions and view-related
//! logic to separate the presentation layer from the core application state.

pub mod debug;
pub mod inputs;
pub mod keymap;
pub mod menu;
pub mod panels;
pub mod settings;
pub mod startup;
pub mod view;
pub use components::SidebarMsg;
pub mod clipboard;
pub mod conflict_policy;
pub mod dialog;
pub mod sidebar_network;
pub mod tabs;
pub mod undo_redo;
pub use tabs::TabState;

mod components;
mod doc_css_variables;
mod help;

// --- 3. Public Re-exports ---
// This allows main.rs to call `ui::FluxApp`
pub use components::{FileItem, SidebarPlace, ThumbEvent, ThumbHook};
#[allow(unused_imports)]
// used by `src/lib.rs` for `gen-docs`, not linked by the `flux-fm` binary alone
pub use doc_css_variables::render_css_variables_markdown;
pub use help::HelpWindow;

/// Groups UI-specific constants such as default window dimensions or CSS class names.
pub mod constants {
    // Defined in `domain` so that `app` and `utils` need not import `ui`; re-exported
    // here so `ui::constants::*` paths keep working. Not every name is used by `ui` itself.
    #[allow(unused_imports)]
    pub use crate::domain::model::constants::{
        CRISP_ICON_SIZES, DEFAULT_HEIGHT, DEFAULT_WIDTH, MAX_HISTORY, MAX_RECENT_ITEMS, MIME_DIR,
        RECENT_URI, TEMPLATE_CWD, TEMPLATE_PATHS, VIEW_FILTER, VIEW_PATH, VIEW_SEARCH,
    };

    // CSS Classes
    pub const CARD_CSS_CLASS: &str = "flux-card";
    pub const SIDEBAR_CSS_CLASS: &str = "sidebar";
    pub const SIDEBAR_ROOT_CSS_CLASS: &str = "flux-sidebar-root";
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
    pub const VIEW_ENTRY: &str = "entry";
    pub const VIEW_LABEL: &str = "label";
    pub const FILTER_BAR_CSS_CLASS: &str = "flux-filter-bar";

    // Layout Constraints
    pub const SCROLLED_WINDOW_MIN_WIDTH: i32 = 480;
    pub const LOCATION_ENTRY_WIDTH_REQUEST: i32 = 450;
    pub const SEARCH_ENTRY_WIDTH_REQUEST: i32 = 450;
    pub const RECENT_STACK_CAPACITY: usize = 10;
    pub const SIDEBAR_SPACING: i32 = 18;
    /// How long a drag must stay over the sidebar before the pin zone appears.
    pub const SIDEBAR_PIN_ZONE_DELAY_MS: u64 = 400;
    /// How long a drag must rest on a row before it highlights as a drop target.
    pub const SIDEBAR_ROW_HOVER_DELAY_MS: u64 = 250;
    /// Auto-navigate delay while a drag idles over a folder row.
    pub const SIDEBAR_AUTONAV_DELAY_MS: u64 = 750;
    pub const HEADER_BTN_SPACING: i32 = 6;
    pub const STATUS_ICON_SPACING: i32 = 8;
    pub const HEADER_MARGIN_END: i32 = 12;

    // Widget Specifics
    pub const STATUS_ICON_SIZE: i32 = 16;

    // MIME Types
    pub const MIME_TEXT: &str = "text/plain";
    pub const MIME_EMPTY: &str = "inode/x-empty";

    // Action Filtering Keywords
    pub const FILTER_ALL: &str = "*";
    pub const FILTER_TRASH: &str = "trash";
    pub const FILTER_FOLDER: &str = "folder";
    pub const FILTER_FILE: &str = "file";

    // Shell & Command Templates

    // Icons & URIs
    pub const ICON_BACK: &str = "go-previous-symbolic";
    pub const ICON_FORWARD: &str = "go-next-symbolic";
    pub const ICON_TRASH: &str = "user-trash-full-symbolic";
    pub const ICON_SORT_INDICATOR: &str = "view-sort-ascending-symbolic";
    pub use crate::infra::trash::TRASH_URI;

    // Mouse Buttons
    pub const MOUSE_BACK: u32 = 8;
    pub const MOUSE_FORWARD: u32 = 9;
    pub const MOUSE_RIGHT_CLICK: u32 = 3;
    pub const MOUSE_MIDDLE: u32 = 2;

    // Text Labels & Tooltips
    pub const BREADCRUMB_MAX_WIDTH_CHARS: u32 = 20;
    pub const SWIPE_VELOCITY_THRESHOLD: f64 = 500.0;
}
