//! Persisted configuration setters.

use crate::domain::model::BackgroundSlot;
use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SettingsMsg {
    /// Updates the global UI scale factor across the application.
    SetUiScale(f64),
    /// Updates the tint overlay opacity for a background slot and re-renders the CSS.
    SetBackgroundAlpha { slot: BackgroundSlot, alpha: f64 },
    /// Deletes all custom background image files from the local data directory
    /// and resets all background styling across the window and sidebars.
    ClearFluxBackgrounds,
    /// Copies an image file to the designated application background slot
    /// (`~/.local/share/flux/data/resources/images/`) and dynamically injects
    /// the updated CSS provider styles to refresh the view immediately.
    SetFluxBackground {
        target: PathBuf,
        slot: BackgroundSlot,
    },
    /// Toggles dynamic scaling of file label fonts with grid icon size.
    SetScaleFontWithIcons(bool),
    /// Updates the list of file extensions hidden from grid labels and refreshes the view.
    SetHiddenExtensions(Vec<String>),
    /// Updates the single-click activation setting.
    SetSingleClick(bool),
    /// Updates the global hidden files visibility.
    SetShowHidden(bool),
    /// Updates the folders-first sorting priority.
    SetFoldersFirst(bool),
    /// Sets the item-count threshold above which folders open in list mode.
    /// `0` disables the auto-switch. Persisted in the config file.
    SetAutoListThreshold(usize),
    /// Toggles Client-Side Decorations (CSD).
    SetShowCsd(bool),
    /// Toggles visibility of standard XDG directories in the sidebar.
    SetShowXdgDirs(bool),
    /// Updates the active UI theme name.
    SetTheme(Option<String>),
    /// Updates a specific keyboard shortcut.
    SetShortcut(String, Option<String>),
    /// Updates the pixel spacing between items in the grid.
    SetGridSpacing(i32),
    /// Updates the character limit for file labels.
    SetMaxWidthChars(i32),
    /// Updates how many path segments the header breadcrumbs show.
    SetBreadcrumbCount(usize),
    /// Updates the character limit for breadcrumb labels.
    SetBreadcrumbMaxChars(i32),
    /// Toggles multi-line label wrapping in the grid.
    SetExpandLabels(bool),
    /// Toggles generation of `~/.config/fish/conf.d/flux-theme.fish`.
    /// Off by default, the file is inert and can be deleted when off.
    SetFishTheme(bool),
    /// Shows or hides the filter button in the header bar.
    SetHeaderShowFilter(bool),
    /// Shows or hides the sort indicator in the header bar.
    SetHeaderShowSort(bool),
    /// Shows or hides the list/grid toggle in the header bar.
    SetHeaderShowView(bool),
    /// Shows or hides the preview-panel toggle in the header bar.
    SetHeaderShowPreview(bool),
    /// Requires Ctrl to be held for sidebar context menus to open.
    SetSidebarMenuRequiresCtrl(bool),
    /// Requires Ctrl to be held for preview-panel and quick-list context menus to open.
    SetPanelMenuRequiresCtrl(bool),
}
