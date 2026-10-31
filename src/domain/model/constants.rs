//! Toolkit-free constants shared by the update layer, the config defaults and the UI.

/// Default width of a new main window, in pixels.
pub const DEFAULT_WIDTH: i32 = 1100;
/// Default height of a new main window, in pixels.
pub const DEFAULT_HEIGHT: i32 = 750;

/// Header stack child that shows the location (breadcrumb) bar.
pub const VIEW_PATH: &str = "path";
/// Header stack child that shows the search entry.
pub const VIEW_SEARCH: &str = "search";
/// Header stack child that shows the filter bar.
pub const VIEW_FILTER: &str = "filter";
/// Maximum number of entries kept in a tab's navigation history.
pub const MAX_HISTORY: usize = 100;
/// Maximum number of entries kept in the Recents view.
pub const MAX_RECENT_ITEMS: usize = 9;

/// MIME type of a directory.
pub const MIME_DIR: &str = "inode/directory";

/// Command-template placeholder that expands to the selected paths.
pub const TEMPLATE_PATHS: &str = "%p";
/// Command-template placeholder that expands to the current directory.
pub const TEMPLATE_CWD: &str = "%d";

/// Default accelerator that focuses the search entry.
pub const SEARCH_SHORTCUT: &str = "<ctrl>f";

/// URI of the virtual Recents folder.
pub const RECENT_URI: &str = "recent:///";
/// Icon sizes, in pixels, that the zoom steps snap to.
pub const CRISP_ICON_SIZES: &[i32] = &[
    16, 24, 32, 48, 64, 96, 128, 144, 160, 192, 256, 384, 512, 768,
];
