use adw::gdk;
use gtk::gio;
use relm4::factory::FactoryVecDeque;
use relm4::typed_view::grid::TypedGridView;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::sync::OnceLock;

use crate::ui::{FileItem, SidebarPlace};

/// Global communication channel for sending messages to the main application loop from background threads.
pub static SENDER: OnceLock<relm4::Sender<AppMsg>> = OnceLock::new();

fn default_true() -> bool {
    true
}

/// Internal metadata container for parallel directory processing.
#[derive(Debug, Clone)]
pub struct FileLoadContext {
    pub display_name: String,
    pub target_path: PathBuf,
    pub size: u64,
    pub mtime: i64,
    pub is_dir: bool,
    pub sort_name: String,
    pub thumbnail_path: Option<PathBuf>,
}

/// Represents a single component of a filesystem path for breadcrumb navigation.
#[derive(Debug, Clone)]
pub struct PathSegment {
    pub name: String,
    pub path: PathBuf,
}

/// Defines a user-configured external command to be displayed in context menus.
#[derive(Clone, Debug)]
pub struct CustomAction {
    pub label: String,
    pub submenu: Option<String>,
    pub action_name: String,
    pub command: String,
    pub mime_types: Vec<String>,
}

/// Top-level configuration structure for persistent application settings.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub ui: UIConfig,
    pub sidebar: Vec<CustomPlace>,
}

/// Available sorting criteria for the file view.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortBy {
    #[default]
    Name,
    Date,
    Size,
}

/// Metadata for actions available within a specific UI context.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ContextAction {
    pub label: String,
    pub action_name: String,
    pub command: String,
    pub mime_types: Vec<String>,
}

/// Visual and behavioral settings for the User Interface.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UIConfig {
    pub default_icon_size: i32,
    pub sidebar_width: i32,
    pub show_xdg_dirs: bool,
    pub single_click: bool,
    pub theme: Option<String>,
    #[serde(default)]
    pub default_sort: SortBy,
    #[serde(default = "default_true")]
    pub folders_first: bool,
    pub show_hidden_by_default: bool,
    pub show_xdg_dirs_by_default: bool,
    pub folder_sort: HashMap<String, SortBy>,
    pub folder_icon_size: HashMap<String, i32>,
    pub device_renames: HashMap<String, String>,
}

/// A user-defined location entry for the sidebar bookmarks.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomPlace {
    pub name: String,
    pub icon: String,
    pub path: String,
}

/// The primary state container for the Flux application.
#[derive(Debug)]
pub struct FluxApp {
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
    /// Flag indicating the search interface was just initialized to trigger focus.
    pub search_just_opened: bool,
    pub current_icon_size: i32,
    /// Factory-managed collection of breadcrumb segments for the header.
    pub breadcrumbs: FactoryVecDeque<PathSegment>,
    /// A pinned list of directories for quick multi-context switching.
    pub exclusive_list: Vec<PathBuf>,
    pub exclusive_index: Option<usize>,
    pub context_menu_popover: gtk::PopoverMenu,
    pub menu_actions: Vec<CustomAction>,
    pub active_item_path: Option<PathBuf>,
    pub directory_monitor: Option<gio::FileMonitor>,
    pub action_group: gio::SimpleActionGroup,
    pub sort_by: SortBy,
    pub show_hidden: bool,
    pub config: Config,
    pub _volume_monitor: gio::VolumeMonitor,
    /// Current search/filter string.
    pub filter: String,
    /// Current active header bar state (e.g., "path", "search", "entry").
    pub header_view: String,
}

/// Enumeration of all messages handled by the application's update loop.
#[derive(Debug, Clone)]
pub enum AppMsg {
    /// Calculate coordinates and determine target for a context menu.
    PrepareContextMenu(f64, f64, Option<PathBuf>),
    /// Display the context menu popover with relevant actions for the given mime type.
    ShowContextMenu {
        x: f64,
        y: f64,
        path: Option<PathBuf>,
        mime: String,
    },
    #[allow(dead_code)]
    OpenFileProperties(PathBuf),
    /// Update the current path and refresh the file list.
    Navigate(PathBuf),
    /// Finalize a file or directory rename operation.
    PerformRename(PathBuf, String),
    /// Navigate to a specific index in the recent folders stack.
    JumpToRecent(usize),
    /// Move forward or backward through the recent stack.
    CycleRecent(i32),
    /// Append a character to the active search filter.
    SearchInput(char),
    /// Remove the last character from the active search filter.
    SearchBackspace,
    /// Synchronize the search entry state with the model.
    CloseSearchSync,
    /// Add the current directory to the exclusive navigation list.
    AddExclusive,
    /// Clear all items from the exclusive navigation list.
    ClearExclusive,
    /// Switch to the next directory in the exclusive list.
    NextExclusive,
    /// Switch to the previous directory in the exclusive list.
    PrevExclusive,
    /// Enter inline rename mode for the specified file.
    StartRename(PathBuf),
    /// Execute the primary action for all currently selected items.
    Activate,
    /// Trigger the rename state for the currently selected item.
    TriggerRenameSelection,
    #[allow(dead_code)]
    ToggleSingleClick,
    /// Force a rebuild of the sidebar entries.
    RefreshSidebar,
    /// Open the keyboard shortcuts and help overlay.
    ShowHelp,
    /// Handle a Drag-and-Drop move/copy operation for multiple items.
    HandleDrop {
        source_paths: Vec<PathBuf>,
        dest_path: PathBuf,
    },
    /// Toggle visibility of dotfiles and hidden items.
    ToggleHidden,
    /// Rotate through available sorting methods.
    CycleSort,
    /// Toggle between "Folders First" and mixed sorting.
    CycleFolderPriority,
    /// Update the string used to filter the current view.
    UpdateFilter(String),
    /// Signal that a thumbnail has been successfully generated.
    ThumbnailReady {
        name: String,
        texture: gdk::Texture,
        load_id: u64,
    },
    /// Switch the header bar between path, entry, and search modes.
    SwitchHeader(String),
    /// Execute a shell command.
    ExecuteCommand(String),
    /// Adjust the icon size based on scroll delta.
    Zoom(f64),
    /// Move back in history.
    GoBack,
    /// Move forward in history.
    GoForward,
    /// Reload the current directory from disk.
    Refresh,
    /// Open all currently selected files or navigate to the selected directory.
    Open,
    /// Delete all files within the system trash location.
    EmptyTrash,
    #[allow(dead_code)]
    RestoreItem(PathBuf),
}
