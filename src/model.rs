use crate::ui::keymap::KeyMap;
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
    /// The sanitized name used for UI rendering and label display.
    pub display_name: String,
    /// The absolute filesystem path to the target file or directory.
    pub target_path: PathBuf,
    /// The size of the file in bytes.
    pub size: u64,
    /// The last modified timestamp represented as a Unix epoch.
    pub mtime: i64,
    /// True if the item is a directory, false if it is a file or symlink.
    pub is_dir: bool,
    /// Pre-processed string used for case-insensitive and natural sorting.
    pub sort_name: String,
    /// The path to a cached or generated image representing the file content.
    pub thumbnail_path: Option<PathBuf>,
}

/// Represents a single component of a filesystem path for breadcrumb navigation.
#[derive(Debug, Clone)]
pub struct PathSegment {
    /// The user-facing name of the specific directory in the path hierarchy.
    pub name: String,
    /// The full absolute path representing this specific segment's location.
    pub path: PathBuf,
}

/// Defines a user-configured external command to be displayed in context menus.
#[derive(Clone, Debug)]
pub struct CustomAction {
    /// The text label to be displayed in the context menu.
    pub label: String,
    /// Optional name of a parent menu to group this action under.
    pub submenu: Option<String>,
    /// Unique identifier used to register and trigger the action.
    pub action_name: String,
    /// The shell command string to be executed, supporting path placeholders.
    pub command: String,
    /// A list of supported MIME types or patterns for context-sensitivity.
    pub mime_types: Vec<String>,
}

/// User-defined keyboard shortcuts for core application operations.
#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq, Eq)]
pub struct ShortcutsConfig {
    /// Key combination to exit the application.
    pub quit: Option<String>,
    /// Key combination to open the selected file or enter a directory.
    pub open: Option<String>,
    /// Key combination to move selected items to the trash.
    pub delete: Option<String>,
    /// Key combination to initiate an inline filename edit.
    pub rename: Option<String>,
    /// Key combination to go back in the navigation history.
    pub back: Option<String>,
    /// Key combination to go forward in the navigation history.
    pub forward: Option<String>,
    /// Key combination to reload the current directory contents.
    pub refresh: Option<String>,
    /// Key combination to focus the search interface.
    pub search: Option<String>,
    /// Key combination to display the metadata properties window.
    pub open_properties: Option<String>,
    /// Key combination to toggle the visibility of hidden files.
    pub toggle_hidden: Option<String>,
    /// Key combination to navigate to root directory.
    pub root: Option<String>,
}

/// Top-level configuration structure for persistent application settings.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Visual and behavioral settings for the application window and widgets.
    pub ui: UIConfig,
    /// A collection of user-defined bookmarks and places for the sidebar.
    #[serde(default)]
    pub sidebar: Vec<CustomPlace>,
    /// Custom keybindings for navigating and managing files.
    #[serde(default)]
    pub shortcuts: ShortcutsConfig,
}

/// Available sorting criteria for the file view.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortBy {
    /// Sort items alphabetically by their display name.
    #[default]
    Name,
    /// Sort items by their last modified timestamp.
    Date,
    /// Sort items by their filesystem size in bytes.
    Size,
}

/// Metadata for actions available within a specific UI context.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ContextAction {
    /// The user-facing text displayed in the menu for this action.
    pub label: String,
    /// Unique identifier for the action used by the system to dispatch events.
    pub action_name: String,
    /// The shell command or internal function identifier to execute upon activation.
    pub command: String,
    /// List of file types or categories where this action is valid.
    pub mime_types: Vec<String>,
}

/// Visual and behavioral settings for the User Interface.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct UIConfig {
    /// Default pixel size for file and folder icons.
    pub default_icon_size: i32,
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
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceRename {
    pub name: String,
    pub icon: Option<String>,
}

/// A user-defined location entry for the sidebar bookmarks.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CustomPlace {
    pub name: String,
    pub icon: String,
    pub path: String,
}

/// The primary state container for the Flux application.
#[derive(Debug)]
pub struct FluxApp {
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
    /// Flag indicating the search interface was just initialized to trigger focus.
    pub search_just_opened: bool,
    /// Current pixel size of the grid item icons.
    pub current_icon_size: i32,
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
    /// Current active header bar state (e.g., "path", "search", "entry").
    pub header_view: String,
    /// Reactive string containing selection counts and sizes for the status bar.
    pub selection_status: String,
    /// The completion percentage of the current background task, if any.
    pub task_progress: Option<f64>,
    /// Toast overlay for displaying transient notifications.
    pub toast_overlay: adw::ToastOverlay,
}

/// Enumeration of all messages handled by the application's update loop.
#[derive(Debug, Clone)]
pub enum AppMsg {
    /// Updates the startup window width.
    SetWindowWidth(i32),
    /// Updates the startup window height.
    SetWindowHeight(i32),
    /// Signals that the file selection set in the main grid has changed.
    SelectionChanged,
    /// Updates the single-click activation setting.
    SetSingleClick(bool),
    /// Updates the global hidden files visibility.
    SetShowHidden(bool),
    /// Updates the folders-first sorting priority.
    SetFoldersFirst(bool),
    /// Updates the default icon size.
    SetIconSize(i32),
    /// Updates the preferred sidebar width.
    SetSidebarWidth(i32),
    /// Toggles Client-Side Decorations (CSD).
    SetShowCsd(bool),
    /// Toggles visibility of standard XDG directories in the sidebar.
    SetShowXdgDirs(bool),
    /// Updates the active UI theme name.
    SetTheme(Option<String>),
    /// Updates the default sorting method.
    SetDefaultSort(SortBy),
    /// Updates a specific keyboard shortcut.
    SetShortcut(String, Option<String>),
    /// Copy the current selection to the clipboard with a "copy" intent.
    Copy,
    /// Copy the current selection to the clipboard with a "cut" intent.
    Cut,
    /// Request data from the clipboard and trigger a Move or Copy.
    Paste,
    /// Triggers the process to move selected files to the system trash.
    Delete,
    /// Internal message to execute the file operations after clipboard data is retrieved.
    PerformPaste(Vec<gio::File>),
    /// Launches the currently selected file(s) using a specific application.
    ///
    /// This variant bypasses thread-safety restrictions of `gio::AppInfo` by passing
    /// the application's Desktop ID (e.g., "org.gnome.gedit.desktop") as a `String`.
    /// The actual `GAppInfo` is resolved on the main thread within the update loop
    /// using `gio::DesktopAppInfo::new()`.
    LaunchWithApp(String),
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
    /// Handles cross-instance file moves via `text/uri-list` serialization.
    HandleExternalDrop {
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
    /// Updates the configuration to reflect whether the window is currently maximized.
    SetMaximized(bool),
    /// The path of a file that has been removed from the filesystem.
    FileDeleted(std::path::PathBuf),
    /// The path of a file whose contents or metadata have been modified.
    FileChanged(std::path::PathBuf),
    /// The completion percentage of a background task, represented as a value from 0.0 to 1.0.
    TaskProgress(f64),
    /// Indicates that the current background task has finished execution.
    TaskCompleted,
    /// This variant is used to provide brief, non-blocking feedback to the user
    ShowToast(String),
}
