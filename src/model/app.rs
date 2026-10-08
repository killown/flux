use crate::model::{CachedFolder, Config, CustomAction, PathSegment, SortBy};
use crate::ui::keymap::KeyMap;
use crate::ui::{FileItem, SidebarPlace};
use relm4::factory::FactoryVecDeque;
use relm4::typed_view::grid::TypedGridView;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// The primary state container for the Flux application.
#[derive(Debug)]
pub struct FluxApp {
    /// Revealer widget wrapping the right location panel.
    pub location_panel_revealer: Option<gtk::Revealer>,
    /// Whether the right location panel is currently visible.
    pub location_panel_visible: bool,
    /// Whether the right location panel widgets have been instantiated.
    pub location_panel_initialized: bool,
    /// Path entry inside the location panel, refreshed on each open.
    pub location_entry: Option<gtk::Entry>,
    /// Sliding revealer widget that wraps the right-side git diff review panel.
    pub diff_panel_revealer: Option<gtk::Revealer>,
    /// Whether the git diff review panel is currently expanded and visible.
    pub diff_panel_visible: bool,
    /// Indicates if the diff panel widget tree has been constructed and bound.
    pub diff_panel_initialized: bool,
    /// Text buffer containing the formatted and syntax-highlighted git patch.
    pub diff_text_buffer: Option<gtk::TextBuffer>,
    /// File path currently displayed inside the git diff review panel.
    pub active_diff_target: Option<PathBuf>,
    /// Scanned directory for `git_status_map`.
    pub git_status_dir: PathBuf,
    /// Git file status lookup table for entries in `git_status_dir`.
    pub git_status_map: std::collections::HashMap<PathBuf, crate::services::git::GitFileStatus>,
    /// Tracks if active directory belongs to a git repository.
    pub is_in_git_repo: bool,
    /// Timestamps of recent autoplay launches, used to enforce the launch-rate
    /// limit that prevents GStreamer pipeline FD exhaustion under rapid selection.
    pub video_preview_launches: std::collections::VecDeque<std::time::Instant>,
    /// Set when the launch-rate limit trips, autoplay is suppressed until this instant.
    pub video_preview_cooldown_until: Option<std::time::Instant>,
    /// Container widget that holds and transitions between multiple tab pages.
    pub tab_view: adw::TabView,
    /// Header strip displaying open tabs, automatically hidden when only one tab is open.
    pub tab_bar: adw::TabBar,
    /// Collection of isolated navigation states for all active tabs.
    pub tabs: Vec<crate::ui::TabState>,
    /// Zero-based index of the currently active tab within `tabs`.
    pub active_tab_index: usize,
    /// Monotonically increasing identifier generator for assigning unique IDs to new tabs.
    pub next_tab_id: u64,
    /// Manages bounded concurrency and viewport-based cancellation tokens for lazy thumbnail generation tasks.
    pub thumbnail_manager: Arc<crate::services::thumbnails::ThumbnailTaskManager>,
    /// Revealer widget wrapping the right tag navigator sidebar panel.
    pub tag_panel_revealer: Option<gtk::Revealer>,
    /// Whether the lazy-initialized right tag navigator sidebar panel is currently visible.
    pub tag_panel_visible: bool,
    /// Whether the right tag navigator sidebar panel widgets have been instantiated.
    pub tag_panel_initialized: bool,
    /// Revealer widget wrapping the right search sidebar panel.
    pub search_panel_revealer: Option<gtk::Revealer>,
    /// Whether the lazy-initialized right search sidebar panel is currently visible.
    pub search_panel_visible: bool,
    /// Whether the right search sidebar panel widgets have been instantiated.
    pub search_panel_initialized: bool,
    /// Line number of the item targeted by the last context menu. 0 when not a content-search hit.
    pub active_item_line: usize,
    /// True while showing results (or no-results) from the advanced search dialog, cleared on navigate or reset.
    #[allow(dead_code)]
    pub last_search_was_advanced: bool,
    /// Whether the bottom status bar is currently visible.
    pub statusbar_visible: bool,
    /// Tracks whether the file grid scrollable container has reached the bottom boundary.
    pub scrolled_to_bottom: bool,
    /// Whether the top header bar is currently visible.
    pub header_visible: bool,
    /// Weak or cloned handle to the header bar widget for toggling visibility dynamically.
    pub header_widget: Option<gtk::Widget>,
    /// Path of the video currently playing inline in the selected card.
    pub active_video_preview: Option<PathBuf>,
    /// Active GLib timeout source ID used to debounce rapid selection changes for video previews.
    pub video_preview_source: Option<glib::SourceId>,
    /// In-memory folder session cache mapped by directory path.
    pub folder_cache: std::collections::HashMap<PathBuf, CachedFolder>,
    /// In-session undo/redo history for file operations.
    pub file_op_history: crate::ui::undo_redo::FileOpHistory,
    /// Handle to the active command output dialog, if open.
    pub command_dialog: Option<crate::ui::dialog::command::CommandDialogHandle>,
    /// Weak reference to the inline header path entry for live text sync.
    pub header_path_entry: glib::WeakRef<gtk::Entry>,
    // Search state for content search mode.
    pub search_saved_layout: bool,
    /// Maximum grid columns before content search, restored when search ends.
    pub saved_max_columns: u32,
    /// The user's preferred list‑mode state before a content search forced it to `true`.
    pub saved_list_mode: bool,
    /// Indicates if an active content search is currently running.
    pub is_content_searching: bool,
    /// Token to cancel the currently active content search operation.
    pub content_search_cancellable: Option<gio::Cancellable>,
    pub network_section: gtk::Box,
    /// True while the current archive path requires a password that has not yet been supplied.
    pub archive_locked: bool,
    /// Cached password for accessing encrypted subdirectories within the current archive session.
    pub cached_archive_password: Option<String>,
    /// The current label for the contextual Recents button.
    pub recents_label: String,
    /// The current tooltip for the contextual Recents button.
    pub recents_tooltip: String,
    /// Used to toggle the header button label.
    pub recents_has_selection: bool,
    /// The Paned widget that contains the terminal.
    pub terminal_paned: Option<gtk::Paned>,
    /// Whether the terminal has been cleared on first open.
    pub terminal_cleared: bool,
    /// The embedded VTE terminal widget.
    pub terminal: crate::services::terminal::Terminal,
    /// Tracks whether the terminal's PTY shell process has been spawned.
    pub terminal_spawned: bool,
    /// Whether the terminal panel is currently visible.
    pub terminal_visible: bool,
    /// Indicates whether a background file system operation or directory reload is currently in progress.
    pub is_loading: bool,
    /// Persistent SQLite-backed manager for application state and metadata.
    pub state_db: Arc<crate::services::db::StateManager>,
    /// Circular buffer of recently visited locations.
    pub recent_stack: VecDeque<PathBuf>,
    /// The primary grid view component displaying file items.
    pub files: TypedGridView<FileItem, gtk::MultiSelection>,
    /// Factory-managed collection of sidebar navigation entries.
    pub sidebar: FactoryVecDeque<SidebarPlace>,
    /// The current directory being browsed.
    pub current_path: PathBuf,
    /// Backwards navigation history.
    pub history: Vec<PathBuf>,
    /// Forwards navigation history (filled when moving back).
    pub forward_stack: Vec<PathBuf>,
    /// Monotonically increasing ID to synchronize asynchronous thumbnail/file loading.
    pub load_id: Arc<AtomicU64>,
    /// Grid indices for which a lazy thumbnail request has already been dispatched in
    /// the current session.
    pub pending_thumbnails: std::collections::HashSet<u32>,
    /// Flag indicating the search interface was just initialized to trigger focus.
    pub search_just_opened: bool,
    /// Current pixel size of the grid item icons.
    pub current_icon_size: i32,
    /// Current pixel size of icons when the view is in list mode.
    pub current_list_icon_size: i32,
    /// Factory-managed collection of breadcrumb segments for the header.
    pub breadcrumbs: FactoryVecDeque<PathSegment>,
    /// A pinned list of directories for quick multi-context switching.
    pub exclusive_list: Vec<PathBuf>,
    /// The index of the currently active item in the exclusive directory list.
    pub exclusive_index: Option<usize>,
    /// Floating menu containing context-sensitive file and application actions.
    pub context_menu_popover: gtk::PopoverMenu,
    /// Collection of user-defined or plugin-provided contextual actions.
    pub menu_actions: Vec<CustomAction>,
    /// The filesystem path of the item currently targeted by a context menu or action.
    pub active_item_path: Option<PathBuf>,
    /// Monitor to receive real-time notifications of changes in the current directory.
    pub directory_monitor: Option<gio::FileMonitor>,
    /// Group of named actions exposed to the UI for activation.
    pub action_group: gio::SimpleActionGroup,
    /// The currently active property used to order the file list.
    pub sort_by: SortBy,
    /// Whether files and directories starting with a dot are displayed.
    pub show_hidden: bool,
    /// Parsed application configuration containing UI and behavioral preferences.
    pub config: Config,
    /// Mapping of keyboard shortcuts to application messages.
    pub keymap: KeyMap,
    /// Monitor for detecting connected storage devices and mount changes.
    pub _volume_monitor: gio::VolumeMonitor,
    /// Current search/filter string.
    pub filter: String,
    /// Session-scoped glob pattern allowlist for persistent navigation filtering.
    pub extension_filter: Option<Vec<String>>,
    /// Precompiled glob matcher for the current extension filter.
    pub extension_globset: Option<globset::GlobSet>,
    /// When true the file grid renders items as compact horizontal rows
    /// (small icon + filename) instead of the default vertical card layout.
    pub is_list_mode: bool,
    /// Current active header bar state (e.g., "path", "search", "entry").
    pub header_view: String,
    /// Reactive string containing selection counts and sizes for the status bar.
    pub selection_status: String,
    /// The completion percentage of the current background task, if any.
    pub task_queue: Arc<crate::services::tasks::TaskQueue>,
    /// Toast overlay for displaying transient notifications.
    pub toast_overlay: adw::ToastOverlay,
    /// Tracks the currently displayed toast so it can be dismissed before showing a new one.
    pub last_toast: Option<adw::Toast>,
    /// Determines if the file list is sorted in ascending (true) or descending (false) order.
    pub sort_ascending: bool,
    // sidebar visibility state, used to restore the previous state when toggling.
    pub sidebar_visible: bool,
    /// Reference to the sidebar widget for toggling visibility.
    pub sidebar_widget: Option<gtk::Widget>,
    /// Horizontal box holding the quick-list tab buttons.
    ///
    /// Populated imperatively by `RebuildQuickPanel` whenever `exclusive_list` changes.
    /// Lives outside the relm4 view macro so it can be mutated freely from `update.rs`.
    pub quick_panel_box: gtk::Box,
    /// Handle to the active file-transfer progress dialog, if open.
    ///
    /// `None` while no dialog is showing, `Some` while at least one transfer
    /// is active and the dialog threshold has been reached.
    pub transfer_dialog: Option<crate::ui::dialog::transfer::TransferDialogHandle>,
    /// True while a per-file conflict-resolution dialog is blocking a worker.
    /// Prevents `handle_show_transfer_dialog` from opening the transfer
    /// progress window on top of the conflict prompt.
    pub conflict_dialog_active: bool,
}
