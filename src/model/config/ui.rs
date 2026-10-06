use crate::model::defaults::{
    default_accent_color, default_auto_show_diff, default_bg_alpha,
    default_content_search_max_file_mb, default_diff_editor, default_diff_panel_width,
    default_ffmpeg_seek_seconds, default_ffmpeg_threads, default_folder_cache_capacity,
    default_loader_batch_size, default_max_history, default_max_search_results,
    default_mime_font_size, default_search_panel_width, default_tag_panel_width,
    default_thumbnail_size, default_thumbnail_threads, default_true, default_ui_scale,
};
use crate::model::{DeviceRename, SortBy, TerminalConfig, ThumbnailTypes};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Visual and behavioral settings for the User Interface.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct UIConfig {
    /// Editor binary or command used when opening diff hunk locations.
    #[serde(default = "default_diff_editor")]
    pub diff_editor: String,
    /// Width in pixels allocated for the right-side git diff review sidebar.
    #[serde(default = "default_diff_panel_width")]
    pub diff_panel_width: i32,
    /// Automatically slides open the diff panel whenever a modified git file is selected.
    #[serde(default = "default_auto_show_diff")]
    pub auto_show_diff: bool,
    /// Global UI scale multiplier applied via font DPI and CSS variables.
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f64,
    /// Skip content-search files larger than this (in MiB). 0 = no cap.
    #[serde(default = "default_content_search_max_file_mb")]
    pub content_search_max_file_mb: u64,
    /// Enable the FTS5 filename indexer.
    #[serde(default)]
    pub enable_file_indexing: bool,
    /// Tint overlay opacity for the main window background image (0.0 to 1.0).
    #[serde(default = "default_bg_alpha")]
    pub bg_alpha_window: f64,
    /// Tint overlay opacity for the left sidebar background image (0.0 to 1.0).
    #[serde(default = "default_bg_alpha")]
    pub bg_alpha_sidebar_left: f64,
    /// Tint overlay opacity for the right panel background image (0.0 to 1.0).
    #[serde(default = "default_bg_alpha")]
    pub bg_alpha_sidebar_right: f64,
    /// Whether file label font size scales dynamically when resizing/zooming grid icons.
    #[serde(default)]
    pub scale_font_with_icons: bool,
    /// File extensions to hide from grid labels (e.g. `["desktop", "AppImage"]`).
    #[serde(default)]
    pub hidden_extensions: Vec<String>,
    /// Default width of the right tag navigator panel in pixels.
    #[serde(default = "default_tag_panel_width")]
    pub tag_panel_width: i32,
    /// Default width of the right search panel in pixels.
    #[serde(default = "default_search_panel_width")]
    pub search_panel_width: i32,
    /// Show a symbolic link badge and target hints in file views.
    #[serde(default = "default_true")]
    pub show_symlink_emblem: bool,
    /// Persisted across sessions, whether the header bar is visible. Defaults to `true`.
    #[serde(default = "default_true")]
    pub header_visible: bool,
    /// Hex color code used for the base page body of auto-generated extension icons (e.g. `"#e4e4e4"`).
    pub auto_mime_body_color: String,
    /// Hex color code used for the extension text label inside auto-generated icons (e.g. `"#ffffff"`).
    pub auto_mime_font_color: String,
    /// Base font size used for auto-generated document icons.
    #[serde(default = "default_mime_font_size")]
    pub auto_mime_font_size: f64,
    /// When true, automatically generates and caches an SVG document icon if the active theme lacks one.
    #[serde(default = "default_true")]
    pub auto_generate_mime_icons: bool,
    /// Default accent color used for auto-generated document icons.
    #[serde(default = "default_accent_color")]
    pub auto_mime_accent_color: String,
    /// When true, selecting a single video card plays a muted loop preview.
    #[serde(default = "default_true")]
    pub autoplay_video_previews: bool,
    /// Target dimension (in pixels) for generated and cached media preview thumbnails.
    #[serde(default = "default_thumbnail_size")]
    pub thumbnail_size: i32,
    /// When true, window controls appear on the left instead of the right.
    #[serde(default)]
    pub window_controls_left: bool,
    /// Show a small emblem next to the label of directories that contain no visible items.
    #[serde(default)]
    pub show_empty_dir_emblem: bool,
    /// Number of threads per FFmpeg child process when extracting video frames.
    #[serde(default = "default_ffmpeg_threads")]
    pub ffmpeg_threads: usize,
    /// Initial seek position in seconds when grabbing a video thumbnail frame.
    #[serde(default = "default_ffmpeg_seek_seconds")]
    pub ffmpeg_seek_seconds: f64,
    /// Whether FFmpeg should automatically rotate portrait/smartphone video thumbnails.
    #[serde(default)]
    pub ffmpeg_auto_rotate: bool,
    /// Number of items dispatched per chunk into the UI grid.
    #[serde(default = "default_loader_batch_size")]
    pub loader_batch_size: usize,
    /// Number of recently visited directory snapshots kept in memory.
    #[serde(default = "default_folder_cache_capacity")]
    pub folder_cache_capacity: usize,
    /// Maximum concurrent background thumbnail generation workers.
    #[serde(default = "default_thumbnail_threads")]
    pub thumbnail_threads: usize,
    /// Maximum matches allowed during recursive filename/extension search.
    #[serde(default = "default_max_search_results")]
    pub max_search_results: usize,
    /// Maximum history entries retained for back/forward navigation.
    #[serde(default = "default_max_history")]
    pub max_history: usize,
    /// Maximum number of results returned by content search.
    /// Higher values may impact performance on large directories.
    #[serde(default)]
    pub max_content_search_results: usize,
    /// Per-file custom image overrides, keyed by absolute file path string.
    ///
    /// Stores the absolute path to the custom image file (PNG, JPG, WebP, SVG)
    /// that should be rendered as the icon for the corresponding file entry.
    /// Unlike `folder_icons`, this supports any file type, not just directories.
    #[serde(default)]
    pub file_icons: HashMap<String, String>,
    /// Configuration settings for the embedded terminal widget.
    pub terminal: TerminalConfig,
    /// Default pixel size for file and folder icons.
    pub default_icon_size: i32,
    /// Default pixel size for icons when the view is in list mode.
    pub list_icon_size: i32,
    /// Width of the left navigation sidebar in pixels.
    pub sidebar_width: i32,
    /// Whether to display standard XDG user directories like Documents and Downloads.
    pub show_xdg_dirs: bool,
    /// Whether a single click activates an item instead of a double click.
    pub single_click: bool,
    /// Optional name of the custom GTK/Libadwaita theme to apply.
    pub theme: Option<String>,
    /// Global fallback sorting method for file listings.
    #[serde(default)]
    pub default_sort: SortBy,
    /// Whether directories should always be grouped above files.
    #[serde(default = "default_true")]
    pub folders_first: bool,
    /// Directory-specific overrides for the folders-first grouping behavior.
    #[serde(default)]
    pub current_folders_first: std::collections::HashMap<String, bool>,
    /// Whether to show dotfiles and hidden items on application startup.
    #[serde(default)]
    pub show_hidden_by_default: bool,
    /// Directory-specific overrides for the sorting method.
    #[serde(default)]
    pub folder_sort: HashMap<String, SortBy>,
    /// Directory-specific overrides for the icon scale.
    #[serde(default)]
    pub folder_icon_size: HashMap<String, i32>,
    /// Custom display names for detected hardware devices and partitions.
    #[serde(default)]
    pub device_renames: HashMap<String, DeviceRename>,
    /// Whether to render Client-Side Decorations (header bar buttons) within the window.
    pub show_csd: bool,
    /// Whether the application window opens in a maximized state.
    pub start_maximized: bool,
    /// Initial width of the application window in pixels.
    pub startup_window_width: i32,
    /// Initial height of the application window in pixels.
    pub startup_window_height: i32,
    /// Maximum number of characters to display in file labels before truncation.
    pub max_width_chars: i32,
    /// Pixel spacing between items in the file grid.
    pub grid_spacing: i32,
    /// Whether the directory listing is sorted in ascending order.
    pub ascending: bool,
    /// Whether filenames in the grid wrap across multiple lines instead of truncating.
    #[serde(default)]
    pub expand_labels: bool,
    /// Per-path custom icon overrides for directories, keyed by absolute path string.
    #[serde(default)]
    pub folder_icons: HashMap<String, String>,
    /// Persisted across sessions, defaults to `true`.
    #[serde(default = "default_true")]
    pub sidebar_visible: bool,
    /// Whether the Recents virtual location is shown in the sidebar.
    ///
    /// Mirrors the behaviour of Nautilus and Thunar: a single "Recents" row that
    /// opens a live view of the GTK recent-files registry.
    #[serde(default = "default_true")]
    pub show_recents: bool,
    /// Zero-based insertion index for the Recents row within the `[[sidebar]]` entry list.
    ///
    /// `0` places Recents above all custom sidebar entries (the previous hardcoded behaviour).
    /// Any value ≥ the number of `[[sidebar]]` entries appends it after all of them.
    /// This field has no effect when `show_recents` is `false`.
    #[serde(default)]
    pub recents_row: usize,
    /// Global toggle for all thumbnail generation.
    ///
    /// When `false`, no thumbnails are generated for any file type, overriding
    /// the individual `thumbnail_types` settings. Defaults to `true`.
    #[serde(default = "default_true")]
    pub show_thumbnails: bool,
    /// Per-file-type thumbnail generation controls.
    ///
    /// Only effective when `show_thumbnails` is `true`. Allows users to enable
    /// or disable previews for specific file categories independently.
    #[serde(default)]
    pub thumbnail_types: ThumbnailTypes,
    /// Generate thumbnails only for items scrolled into the viewport instead of
    /// loading all of them eagerly when a directory is opened.
    #[serde(default)]
    pub lazy_thumbnails: bool,
    /// When true, drag-and-drop operations for files and folders are completely disabled.
    #[serde(default)]
    pub disable_drag_and_drop: bool,
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            bg_alpha_window: 0.65,
            bg_alpha_sidebar_left: 0.65,
            bg_alpha_sidebar_right: 0.65,
            scale_font_with_icons: false,
            hidden_extensions: Vec::new(),
            show_empty_dir_emblem: false,
            show_symlink_emblem: true,
            file_icons: HashMap::new(),
            default_icon_size: 0,
            list_icon_size: 24,
            sidebar_width: 0,
            show_xdg_dirs: false,
            single_click: false,
            theme: None,
            default_sort: SortBy::default(),
            folders_first: true,
            current_folders_first: HashMap::new(),
            show_hidden_by_default: false,
            folder_sort: HashMap::new(),
            folder_icon_size: HashMap::new(),
            device_renames: HashMap::new(),
            show_csd: false,
            start_maximized: false,
            startup_window_width: 0,
            startup_window_height: 0,
            max_width_chars: 20,
            grid_spacing: 10,
            ascending: true,
            expand_labels: false,
            folder_icons: HashMap::new(),
            terminal: TerminalConfig::default(),
            sidebar_visible: true,
            show_recents: true,
            recents_row: 0,
            show_thumbnails: true,
            thumbnail_types: ThumbnailTypes::default(),
            autoplay_video_previews: true,
            max_content_search_results: crate::services::constants::MAX_CONTENT_SEARCH_RESULTS,
            lazy_thumbnails: false,
            disable_drag_and_drop: false,
            loader_batch_size: default_loader_batch_size(),
            folder_cache_capacity: default_folder_cache_capacity(),
            thumbnail_threads: default_thumbnail_threads(),
            thumbnail_size: 256,
            max_search_results: default_max_search_results(),
            max_history: default_max_history(),
            ffmpeg_threads: default_ffmpeg_threads(),
            ffmpeg_seek_seconds: default_ffmpeg_seek_seconds(),
            ffmpeg_auto_rotate: false,
            window_controls_left: false,
            auto_generate_mime_icons: true,
            auto_mime_accent_color: default_accent_color(),
            auto_mime_font_size: default_mime_font_size(),
            auto_mime_body_color: "#e4e4e4".to_string(),
            auto_mime_font_color: "#ffffff".to_string(),
            header_visible: true,
            search_panel_width: default_search_panel_width(),
            tag_panel_width: default_tag_panel_width(),
            diff_panel_width: default_diff_panel_width(),
            auto_show_diff: default_auto_show_diff(),
            enable_file_indexing: false,
            content_search_max_file_mb: default_content_search_max_file_mb(),
            ui_scale: 1.0,
            diff_editor: default_diff_editor(),
        }
    }
}
