//! Sidebar sections, places and layout.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppSidebarMsg {
    /// Adds a right-panel shortcut (Actions / Search / Tags) to the sidebar
    /// if it isn't already pinned, then refreshes the sidebar.
    AddPanelSidebarEntry {
        name: String,
        path: String,
        icon: String,
    },
    /// Pin a specific tag to the user's sidebar bookmarks in `config.toml`.
    AddTagToSidebar(String),
    /// Prompts a modal dialog to rename a section label.
    PromptSidebarRenameSection {
        old_name: String,
        current_name: String,
    },
    /// Persists a new name for a section label entry in config.toml.
    RenameSidebarSection { old_name: String, new_name: String },
    /// Removes a section label entry from config.toml and refreshes the sidebar.
    RemoveSidebarSection(String),
    /// Prompts the user for a title, then appends a new `kind = "label"`.
    PromptNewSidebarSection,
    /// Appends a new section-label entry with the given title to config.sidebar.
    AddSidebarSection(String),
    /// Show or hide the "Pin to Sidebar" drop zone at the bottom of the sidebar.
    ShowSidebarPinZone(bool),
    /// Prompts a dialog to rename a sidebar bookmark.
    PromptSidebarRename { path: PathBuf, current_name: String },
    /// Persists a new name for a matching sidebar entry in config.toml.
    RenameSidebarPlace { path: PathBuf, new_name: String },
    /// Updates the preferred sidebar width.
    SetSidebarWidth(i32),
    /// Force a rebuild of the sidebar entries.
    RefreshSidebar,
    /// Remove a custom entry from the sidebar by its resolved path.
    RemoveFromSidebar(PathBuf),
    /// Permanently add the selected folder (or current directory) to config.toml sidebar entries.
    AddToSidebarPermanent,
    /// Reorder a custom sidebar entry: move `from` path to the position currently held by `to`.
    ReorderSidebar { from: PathBuf, to: PathBuf },
    /// Pin `path` into the sidebar immediately before the row whose config path matches `before`.
    ///
    /// Emitted when a folder is dragged from the grid and dropped onto a specific sidebar row.
    /// If `before` is not found (e.g. a section label or unmapped mount), appends to the end
    /// of the user-defined sidebar entries.
    PinFolderAt {
        path: PathBuf,
        before: PathBuf,
        label_name: Option<String>,
    },
    /// Move files dragged from the grid and dropped onto a non-folder sidebar row's target folder.
    SidebarDropMove {
        source_paths: Vec<PathBuf>,
        dest_path: PathBuf,
    },
    /// Opens an icon picker dialog for a sidebar entry (symbolic only).
    ShowSidebarIconPicker(PathBuf),
    /// Toggles the visibility of the sidebar.
    ToggleSidebar,
    /// Toggles wide-preview mode: sidebar hidden, file grid squeezed, preview panel fills the rest.
    ToggleWidePreview,
    /// Sets whether the preview panel always opens in wide mode (header button), saved to config.
    SetWidePreviewDefault(bool),
}
