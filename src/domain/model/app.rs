use crate::domain::model::{
    ArchiveState, CachedFolder, Config, CustomAction, Filters, GitState, Panels, PathSegment,
    PreviewState, QuickState, SearchState, SortBy, TerminalState,
};
use crate::ui::keymap::KeyMap;
use crate::ui::{FileItem, SidebarPlace};
use relm4::factory::FactoryVecDeque;
use relm4::typed_view::grid::TypedGridView;
use std::collections::VecDeque;
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
    /// The bottom and full-window terminals.
    pub term: TerminalState,
    /// The pinned directory list and the named Quick Lists behind it.
    pub quick: QuickState,
    /// Password state for encrypted archives browsed this session.
    pub archive: ArchiveState,
    /// Session-scoped filters applied to every directory load.
    pub filters: Filters,
    /// Content-search state and the layout it saves and restores.
    pub search: SearchState,
    /// Git status of the current directory and the file shown in the diff panel.
    pub git: GitState,
    /// Session state of the peek-preview feature outside the panel itself: the prefetch cache,
    /// the wide and full-screen modes, and inline video autoplay.
    pub preview: PreviewState,
    /// Cycle state for `Ctrl+Alt+<letter>`: the anchor directory whose
    /// children are being cycled, the letter, the ordered candidate paths
    /// captured on the first press, and the index of the one we are on
    /// (or about to enter).
    pub alt_cycle: Option<AltCycle>,
    /// `Some` while Navigate mode is on (see [`NavigateMode`]).
    pub navigate: Option<NavigateMode>,
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
    /// Whether the bottom status bar is currently visible.
    pub statusbar_visible: bool,
    /// Tracks whether the file grid scrollable container has reached the bottom boundary.
    pub scrolled_to_bottom: bool,
    /// Whether the top header bar is currently visible.
    pub header_visible: bool,
    /// Weak or cloned handle to the header bar widget for toggling visibility dynamically.
    pub header_widget: Option<gtk::Widget>,
    /// In-memory folder session cache mapped by directory path.
    pub folder_cache: std::collections::HashMap<PathBuf, CachedFolder>,
    /// In-session undo/redo history for file operations.
    pub file_op_history: crate::ui::undo_redo::FileOpHistory,
    /// Handle to the active command output dialog, if open.
    pub command_dialog: Option<crate::ui::dialog::command::CommandDialogHandle>,
    /// Weak reference to the inline header path entry for live text sync.
    pub header_path_entry: glib::WeakRef<gtk::Entry>,
    // Search state for content search mode.
    pub network_section: gtk::Box,
    /// The current label for the contextual Recents button.
    pub recents_label: String,
    /// The current tooltip for the contextual Recents button.
    pub recents_tooltip: String,
    /// Used to toggle the header button label.
    pub recents_has_selection: bool,
    /// Handle to the root `main_stack` (set in `view/init.rs`).
    pub main_stack: Option<gtk::Stack>,
    /// The main window, kept so the toggle socket can hide and show it while it is unmapped.
    pub root_window: Option<adw::Window>,
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
    /// Current pixel size of the grid item icons.
    pub current_icon_size: i32,
    /// Current pixel size of icons when the view is in list mode.
    pub current_list_icon_size: i32,
    /// Factory-managed collection of breadcrumb segments for the header.
    pub breadcrumbs: FactoryVecDeque<PathSegment>,
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
