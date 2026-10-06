/// User-defined keyboard shortcuts for core application operations.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default, PartialEq, Eq)]
pub struct ShortcutsConfig {
    /// Shortcut to open a new tab
    pub new_tab: Option<String>,
    /// Shortcut to close the current tab
    pub close_tab: Option<String>,
    /// Shortcut to switch to the next tab
    pub next_tab: Option<String>,
    /// Shortcut to switch to the previous tab
    pub prev_tab: Option<String>,
    /// Key combination to navigate to the user's home directory.
    pub home: Option<String>,
    /// Key combination to toggle the visibility of the header bar.
    pub toggle_header: Option<String>,
    /// Toggles folder grouping placement (first vs last) for the active folder.
    pub toggle_folders_first: Option<String>,
    /// Copy absolute paths of selected items to clipboard.
    #[serde(default)]
    pub copy_path: Option<String>,
    /// Create symlinks from clipboard paths, auto-rename conflicts.
    #[serde(default)]
    pub create_symlink: Option<String>,
    /// Create hardlinks from clipboard paths (directories not supported).
    #[serde(default)]
    pub create_hardlink: Option<String>,
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
