use crate::domain::model::{CachedFolder, Config, CustomAction, Panels, PathSegment, SortBy};
use crate::ui::keymap::KeyMap;
use crate::ui::{FileItem, SidebarPlace};
use relm4::factory::FactoryVecDeque;
use relm4::typed_view::grid::TypedGridView;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMode {
    #[default]
    All,
    OnlyFolders,
    OnlyFiles,
}

/// State for the `Ctrl+Alt+<letter>` folder cycle.
#[derive(Debug, Clone)]
pub struct AltCycle {
    /// Directory whose children are being cycled.
    pub anchor: PathBuf,
    /// Lowercased letter prefix being cycled.
    pub letter: char,
    /// Candidates captured on the first press, in the loader's sort order.
    pub candidates: Vec<PathBuf>,
    /// Index of the candidate currently entered / last chosen.
    pub index: usize,
    /// Candidates that are archives drawn as folders (`is_virtual_dir`), entered
    /// with `EnterArchive`. Recorded when the candidates are built, because by
    /// the time they are chosen the grid may show another folder or an archive.
    pub virtual_targets: Vec<PathBuf>,
}

/// State for Navigate mode, a session stance where plain letters enter the
/// folders starting with that letter (the `Ctrl+Alt+<letter>` cycle without
/// the chord) instead of filtering.
///
/// Lives on [`FluxApp`], not on a tab and not in `config.toml`: it is entered
/// and left by a chord (or Esc), survives navigation and tab switches, and is
/// gone when Flux closes. The cycle itself is [`AltCycle`].
#[derive(Debug, Clone, Default)]
pub struct NavigateMode {
    /// Folders the cycle was based in before Enter made a deeper folder the
    /// new base, innermost last. Backspace walks back through them.
    pub anchors: Vec<PathBuf>,
    /// Search-bar filter that was active when the mode was entered, put back
    /// on exit.
    pub stashed_filter: Option<String>,
    /// A first-item selection is owed: set on entering the mode and when a
    /// navigation starts inside it, consumed by the next finished folder load.
    /// A refresh never sets it, so it cannot pull the selection back to the top.
    pub select_first: bool,
    /// Path of the item the mode selected on its own, to tell that selection
    /// apart from one the user moved with the arrow keys.
    pub auto_selected: Option<PathBuf>,
}

/// The primary state container for the Flux application.
#[derive(Debug)]
pub struct FluxApp {
    /// Session state of the ten right-side panels.
    pub panels: Panels,
    /// Cycle state for `Ctrl+Alt+<letter>`: the anchor directory whose
    /// children are being cycled, the letter, the ordered candidate paths
    /// captured on the first press, and the index of the one we are on
    /// (or about to enter).
    pub alt_cycle: Option<AltCycle>,
    /// `Some` while Navigate mode is on (see [`NavigateMode`]).
    pub navigate: Option<NavigateMode>,
    /// One-shot: open the preview panel in wide mode after the first folder load (CLI `--preview`).
    pub startup_preview: bool,
    /// True when the current `mime_filter` was applied by the `Space` shortcut,
    /// meaning closing the preview panel should also clear it.
    pub mime_filter_from_space: bool,
    /// Ephemeral MIME filter: only entries whose MIME type equals this are shown.
    pub mime_filter: Option<String>,
    /// Session-scoped folder/file filter applied to every directory load, persists across navigation.
    pub filter_mode: FilterMode,
    /// Bounded LRU of prefetched preview payloads keyed on path + mtime.
    pub preview_cache: std::sync::Arc<crate::app::update::view::prefetch::PreviewCache>,
    /// `Some(sidebar was visible)` while wide-preview mode is active, `None` otherwise.
    pub wide_preview_sidebar: Option<bool>,
    /// `Some((wide mode was already on, window was already full screen))` while full-screen
    /// preview is active, `None` otherwise.
    pub preview_fullscreen: Option<(bool, bool)>,
    /// File path currently displayed inside the git diff review panel.
    pub active_diff_target: Option<PathBuf>,
    /// Scanned directory for `git_status_map`.
    pub git_status_dir: PathBuf,
    /// Git file status lookup table for entries in `git_status_dir`.
    pub git_status_map: std::collections::HashMap<PathBuf, crate::infra::git::GitFileStatus>,
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
    pub thumbnail_manager: Arc<crate::infra::thumbnails::ThumbnailTaskManager>,
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
    /// Passwords resolved this session, keyed by the archive's on-disk path.
    pub cached_archive_passwords: HashMap<PathBuf, String>,
    /// Archives this session has seen demand a password (7z and RAR cannot be probed
    /// cheaply). Paths only, in memory only, feeds the padlock badge.
    pub known_encrypted_archives: std::collections::HashSet<PathBuf>,
    /// Keyring saves the user requested, held back until the archive accepts the password.
    pub pending_keyring_saves: crate::infra::keyring::PendingSaves,
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
    pub terminal: crate::infra::terminal::Terminal,
    /// Tracks whether the terminal's PTY shell process has been spawned.
    pub terminal_spawned: bool,
    /// Whether the terminal panel is currently visible.
    pub terminal_visible: bool,
    /// True while the full-window terminal mode is active. Only meaningful
    /// when `config.ui.terminal.position == TerminalPosition::Fullscreen`.
    pub terminal_fullscreen: bool,
    /// Handle to the root `main_stack` (set in `view/init.rs`).
    pub main_stack: Option<gtk::Stack>,
    /// The main window, kept so the toggle socket can hide and show it while it is unmapped.
    pub root_window: Option<adw::Window>,
    /// Dedicated terminal instance for full-window mode. Lazily created on first
    /// entry, never reparented into the paned or sidebar. Killed on window close.
    pub fullscreen_terminal: Option<crate::infra::terminal::Terminal>,
    /// Handle to the container inside the `"terminal"` Stack page.
    pub fullscreen_terminal_box: Option<gtk::Box>,
    /// True once the fullscreen terminal has spawned its shell at least once.
    /// Independent from `terminal_spawned` (bottom pane) and the sidebar panel.
    pub terminal_spawned_fullscreen: bool,
    /// Indicates whether a background file system operation or directory reload is currently in progress.
    pub is_loading: bool,
    /// Persistent SQLite-backed manager for application state and metadata.
    pub state_db: Arc<crate::infra::db::StateManager>,
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
    /// Every named Quick List. `exclusive_list` and `exclusive_index` always
    /// mirror the selected one, so this copy of the selected list goes stale.
    /// Read it through `FluxApp::quick_snapshot`.
    pub quick_groups: crate::domain::model::quick_groups::QuickGroups,
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
    pub task_queue: Arc<crate::infra::tasks::TaskQueue>,
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
