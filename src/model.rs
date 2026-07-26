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
    /// True when the file's Unix UID does not match the current process's effective UID.
    pub is_foreign_owner: bool,
    /// Whether the filename label should wrap across multiple lines.
    pub expand_labels: bool,
    /// Optional override icon name for the item.
    pub custom_icon: Option<String>,
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
    /// Optional toast message to display after the command is dispatched.
    pub toast: Option<String>,
}

/// User-defined keyboard shortcuts for core application operations.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default, PartialEq, Eq)]
pub struct ShortcutsConfig {
    /// Key combination to open the folder icon picker.
    pub change_icon: Option<String>,
    /// Key combination to reset a folder's icon to default.
    pub reset_icon: Option<String>,
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
    /// Key combination to open the application settings.
    pub settings: Option<String>,
    /// Key combination to open the menu editor.
    pub menu_editor: Option<String>,
    /// Key combination to rotate through available sorting methods.
    pub cycle_sort: Option<String>,
    /// Key combination to toggle between ascending and descending order.
    pub toggle_sort_order: Option<String>,
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
    /// Sort items by their file extension or MIME type.
    Type,
}

impl SortBy {
    /// Returns the `GVariant` string used as the state/target for the stateful
    /// `app.sort-field` radio action, enabling GIO to render the matching menu
    /// item with a native radio checkmark.
    pub fn as_action_state(self) -> gtk::glib::Variant {
        let key = match self {
            SortBy::Name => "name",
            SortBy::Date => "date",
            SortBy::Size => "size",
            SortBy::Type => "type",
        };
        gtk::glib::Variant::from(key)
    }

    /// Reconstructs a [`SortBy`] from the variant key used by the `app.sort-field` action.
    pub fn from_action_key(key: &str) -> Self {
        match key {
            "date" => SortBy::Date,
            "size" => SortBy::Size,
            "type" => SortBy::Type,
            _ => SortBy::Name,
        }
    }
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

/// Per-type thumbnail generation settings for file previews.
///
/// Controls which file types should have visual previews generated in the file grid.
/// Each field defaults to `true` when a new config is created.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ThumbnailTypes {
    /// Generate thumbnails for image files (PNG, JPG, GIF, WebP, etc.)
    pub images: bool,
    /// Generate thumbnails for video files (MP4, MKV, WebM, etc.)
    pub videos: bool,
    /// Generate thumbnails for font files (TTF, OTF, WOFF, etc.)
    pub fonts: bool,
    /// Generate thumbnails for PDF documents
    pub pdfs: bool,
}

impl Default for ThumbnailTypes {
    fn default() -> Self {
        Self {
            images: true,
            videos: true,
            fonts: true,
            pdfs: true,
        }
    }
}

/// Visual and behavioral settings for the User Interface.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct UIConfig {
    /// Configuration settings for the embedded terminal widget.
    pub terminal: TerminalConfig,
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
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            default_icon_size: 0,
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
        }
    }
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
    #[serde(default)]
    pub kind: Option<String>,
    pub icon: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TerminalConfig {
    pub height: i32,
    pub fg_color: String,
    pub bg_color: String,
    pub font: String,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            height: 30,
            fg_color: "#E5E5E5".to_string(),
            bg_color: "#1A1A1A".to_string(),
            font: "JetBrains Mono 13".to_string(),
        }
    }
}

/// The primary state container for the Flux application.
#[derive(Debug)]
pub struct FluxApp {
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
    pub task_queue: Arc<crate::services::tasks::TaskQueue>,
    /// Toast overlay for displaying transient notifications.
    pub toast_overlay: adw::ToastOverlay,
    /// Pending toast messages keyed by action_name, populated at context-menu build time.
    pub pending_toasts: std::collections::HashMap<String, String>,
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
}

/// Enumeration of all messages handled by the application's update loop.
#[derive(Debug, Clone)]
pub enum AppMsg {
    /// Sets the zero-based insertion index for the Recents row within the `[[sidebar]]` entry list.
    ///
    /// Values ≥ the number of configured sidebar entries place Recents after all of them.
    /// Has no effect when `show_recents` is `false`.
    SetRecentsRow(usize),
    /// Remove one or all entries from the recent-files list.
    ClearRecents,
    /// Toggle the embedded terminal panel visibility.
    ToggleTerminal,
    /// Internal trigger to open icon picker (resolves path from selection/current dir).
    TriggerIconPicker,
    /// Internal trigger to reset icon (resolves path from selection/current dir).
    TriggerResetIcon,
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
    PerformPaste { files: Vec<gio::File>, is_cut: bool },
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
    /// Add a directory to the exclusive navigation list.
    ///
    /// `Some(path)` targets an explicit path (e.g. from a context menu action).
    /// `None` resolves the path from the current selection or working directory,
    /// which is the behaviour of the `Insert` keyboard shortcut.
    AddExclusive(Option<PathBuf>),
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
    /// Remove a custom entry from the sidebar by its resolved path.
    RemoveFromSidebar(PathBuf),
    /// Permanently add the selected folder (or current directory) to config.toml sidebar entries.
    AddToSidebarPermanent,
    /// Reorder a custom sidebar entry: move `from` path to the position currently held by `to`.
    ReorderSidebar { from: PathBuf, to: PathBuf },
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
    /// Report progress for a specific background operation slot.
    TaskProgress {
        id: u64,
        current: u64,
        total: u64,
        total_items: usize,
        cancellable: gio::Cancellable,
    },
    /// Signal that a specific background operation has completed.
    TaskCompleted(u64),
    #[allow(dead_code)]
    /// Cancel a single in-flight background operation by its task ID.
    CancelTask(u64),
    /// Cancel every in-flight background operation immediately.
    CancelAllTasks,
    /// Throttled tick to refresh the status bar from the task queue.
    TaskQueueTick,
    /// This variant is used to provide brief, non-blocking feedback to the user
    ShowToast(String),
    /// Delivers the result of an async media duration probe for status bar display.
    ///
    /// Carries `Some(duration)` on success or `None` if the file is not a
    /// media container or `ffprobe` is unavailable.
    MediaDurationReady(Option<std::time::Duration>),
    /// Delivers async file metadata (MIME type and optional image dimensions)
    FileMetaReady {
        mime: String,
        dimensions: Option<(u32, u32)>,
    },
    /// Triggers the asynchronous unmounting of a system drive or mounted volume.
    UnmountDevice(std::path::PathBuf),
    /// Pause a paste operation and ask the user whether to replace conflicting directories.
    ///
    /// Carries the full original file list so it can be resumed via `PerformPasteForced`
    /// and the display names of the conflicting items for the dialog body.
    ConfirmReplacePaste {
        files: Vec<gio::File>,
        conflicts: Vec<String>,
        is_cut: bool,
    },
    /// Resume a paste operation after the user has confirmed directory replacement.
    ///
    /// Identical to `PerformPaste` but skips conflict detection and always passes
    /// `FileCopyFlags::OVERWRITE` for both copy and move operations.
    PerformPasteForced { files: Vec<gio::File>, is_cut: bool },
    /// Updates the pixel spacing between items in the grid.
    SetGridSpacing(i32),
    /// Updates the character limit for file labels.
    SetMaxWidthChars(i32),
    /// Toggles multi-line label wrapping in the grid.
    SetExpandLabels(bool),
    /// Toggles the directory listing between ascending and descending order.
    ToggleSortOrder,
    /// Sets sort direction: true for Ascending, false for Descending.
    SetAsc(bool),
    /// Opens an icon picker dialog for the given directory path.
    ShowIconPicker(PathBuf),
    /// Persists a custom icon name for the given directory path.
    SetFolderIcon { path: PathBuf, icon_name: String },
    /// Removes the custom icon override for the given directory path, restoring the default.
    ResetFolderIcon(PathBuf),
    /// Updates the height of the embedded terminal panel in pixels.
    SetTerminalHeight(i32),
    /// Updates the Pango font description string for the embedded terminal.
    SetTerminalFont(String),
    /// Updates the foreground text color (as a hex string) for the embedded terminal.
    SetTerminalFgColor(String),
    /// Updates the background color (as a hex string) for the embedded terminal.
    SetTerminalBgColor(String),
    /// Opens the About dialog with application metadata and repository link.
    ShowAbout,
    /// Toggles the visibility of the sidebar.
    ToggleSidebar,
    /// Toggles the global thumbnail generation setting.
    ///
    /// When disabled, no thumbnails are generated for any file type, improving
    /// performance on low-spec systems or for users who prefer a minimal view.
    SetShowThumbnails(bool),
    /// Toggles thumbnail generation for a specific file type.
    ///
    /// # Fields
    /// - `type_name`: One of "images", "videos", "fonts", or "pdfs"
    /// - `enabled`: Whether thumbnails should be generated for this type
    SetThumbnailType { type_name: String, enabled: bool },
    /// Toggles the "Recents" virtual entry in the sidebar.
    SetShowRecents(bool),
    /// Remove a single path from the quick-list panel by exact match.
    RemoveQuickItem(PathBuf),
    /// Tear down and reconstruct all tab buttons in the quick-list panel.
    ///
    /// Sent automatically after any mutation of `exclusive_list` so the widget
    /// tree always reflects the current state.
    RebuildQuickPanel,
}

/// Represents a single entry in the Flux context menu configuration.
///
/// This structure maps to the domain-specific language used in `menu.rs`,
/// allowing for the definition of custom actions based on file types.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MenuEntry {
    /// The user-facing text displayed in the menu.
    pub label: String,
    /// An optional category name used to nest this entry within a submenu.
    pub submenu: Option<String>,
    /// Comma-separated patterns (e.g., "image/all", "directory") that define when this entry appears.
    pub mime_types: String,
    /// The shell command to execute, supporting placeholders like %p for the file path.
    pub command: String,
    /// An optional message to display as a toast notification after the command runs.
    pub toast: Option<String>,
}

impl MenuEntry {
    /// Converts the entry into the DSL string format used in `menu.rs`.
    ///
    /// The output follows the pattern:
    /// `"Submenu > Label" => "mime_types", "command", "toast"`
    pub fn to_config_line(&self) -> String {
        let label_field = match &self.submenu {
            Some(sub) => format!("{} > {}", sub, self.label),
            None => self.label.clone(),
        };

        let base = format!(
            r#""{}" => "{}", "{}""#,
            label_field, self.mime_types, self.command
        );

        match &self.toast {
            Some(t) => format!(r#"{}, "{}""#, base, t),
            None => base,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_by_default() {
        let sort = SortBy::default();
        assert_eq!(sort, SortBy::Name);
    }

    #[test]
    fn test_sort_by_serialization() {
        let config = Config {
            ui: UIConfig {
                default_sort: SortBy::Date,
                ..Default::default()
            },
            ..Default::default()
        };

        let toml_str = toml::to_string(&config).expect("Failed to serialize config");
        assert!(toml_str.contains("default_sort = \"Date\""));

        let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
        assert_eq!(parsed.ui.default_sort, SortBy::Date);
    }

    #[test]
    fn test_config_defaults() {
        // Test that an empty TOML string results in a Config with correct defaults
        // specifically testing serde defaults like folders_first
        let empty_toml = "";
        let config: Config = toml::from_str(empty_toml).expect("Failed to parse empty config");

        // Check key defaults
        assert_eq!(config.ui.default_icon_size, 0); // Default for i32 is 0
        assert!(!config.ui.single_click); // Default for bool is false
        assert_eq!(config.ui.default_sort, SortBy::Name); // Custom default via #[default]
        assert!(config.ui.folders_first); // Custom default via serde(default = "default_true")
    }

    #[test]
    fn test_shortcuts_config_serialization() {
        let mut shortcuts = ShortcutsConfig::default();
        shortcuts.back = Some("BackSpace".to_string());
        shortcuts.forward = Some("<Alt>Right".to_string());

        let config = Config {
            shortcuts,
            ..Default::default()
        };

        let toml_str = toml::to_string(&config).expect("Failed to serialize config");
        assert!(toml_str.contains("back = \"BackSpace\""));
        assert!(toml_str.contains("forward = \"<Alt>Right\""));

        let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
        assert_eq!(parsed.shortcuts.back, Some("BackSpace".to_string()));
        assert_eq!(parsed.shortcuts.forward, Some("<Alt>Right".to_string()));
    }

    #[test]
    fn test_custom_place_serialization() {
        let place = CustomPlace {
            name: "Home".to_string(),
            icon: "user-home-symbolic".to_string(),
            path: "~".to_string(),
            kind: None,
        };

        let config = Config {
            sidebar: vec![place],
            ..Default::default()
        };

        let toml_str = toml::to_string(&config).expect("Failed to serialize config");
        assert!(toml_str.contains("name = \"Home\""));
        assert!(toml_str.contains("path = \"~\""));

        let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
        assert_eq!(parsed.sidebar.len(), 1);
        assert_eq!(parsed.sidebar[0].name, "Home");
    }

    #[test]
    fn test_device_rename_serialization() {
        let mut renames = HashMap::new();
        renames.insert(
            "/dev/sda1".to_string(),
            DeviceRename {
                name: "My Disk".to_string(),
                icon: Some("drive-harddisk-symbolic".to_string()),
            },
        );

        let config = Config {
            ui: UIConfig {
                device_renames: renames,
                ..Default::default()
            },
            ..Default::default()
        };

        let toml_str = toml::to_string(&config).expect("Failed to serialize config");
        assert!(toml_str.contains("My Disk"));

        let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
        let renamed = parsed.ui.device_renames.get("/dev/sda1").unwrap();
        assert_eq!(renamed.name, "My Disk");
        assert_eq!(renamed.icon, Some("drive-harddisk-symbolic".to_string()));
    }

    #[test]
    fn test_folder_icons_serialization() {
        let mut icons = HashMap::new();
        icons.insert(
            "/home/user/Projects".to_string(),
            "folder-development-symbolic".to_string(),
        );

        let config = Config {
            ui: UIConfig {
                folder_icons: icons,
                ..Default::default()
            },
            ..Default::default()
        };

        let toml_str = toml::to_string(&config).expect("Failed to serialize config");
        assert!(toml_str.contains("folder-development-symbolic"));

        let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
        assert_eq!(
            parsed.ui.folder_icons.get("/home/user/Projects").cloned(),
            Some("folder-development-symbolic".to_string())
        );
    }

    #[test]
    fn test_thumbnail_types_defaults() {
        let types = ThumbnailTypes::default();
        assert!(types.images);
        assert!(types.videos);
        assert!(types.fonts);
        assert!(types.pdfs);
    }

    #[test]
    fn test_thumbnail_types_serialization() {
        let config = Config {
            ui: UIConfig {
                show_thumbnails: false,
                thumbnail_types: ThumbnailTypes {
                    images: false,
                    videos: true,
                    fonts: false,
                    pdfs: true,
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let toml_str = toml::to_string(&config).expect("Failed to serialize config");
        assert!(toml_str.contains("show_thumbnails = false"));
        assert!(toml_str.contains("images = false"));
        assert!(toml_str.contains("videos = true"));
        assert!(toml_str.contains("fonts = false"));
        assert!(toml_str.contains("pdfs = true"));

        let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
        assert!(!parsed.ui.show_thumbnails);
        assert!(!parsed.ui.thumbnail_types.images);
        assert!(parsed.ui.thumbnail_types.videos);
        assert!(!parsed.ui.thumbnail_types.fonts);
        assert!(parsed.ui.thumbnail_types.pdfs);
    }
}
