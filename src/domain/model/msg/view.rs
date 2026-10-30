//! Selection, sorting, zoom and view toggles.

use crate::domain::model::SortBy;
use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ViewMsg {
    /// Moves the grid selection by `delta` (`-1` = previous, `+1` = next)
    /// along the currently visible (filtered) order, so the preview panel
    /// follows with the next item. Clamps at the ends instead of wrapping.
    SelectionStep(i32),
    /// Toggles the visibility state of the bottom status bar.
    ToggleStatusBar,
    /// Message triggered when the file grid scroll position reaches or leaves the bottom boundary.
    SetScrolledToBottom(bool),
    /// Toggles whether folders are grouped first or last for the current directory.
    ToggleCurrentFoldersFirst,
    /// Signals that the file selection set in the main grid has changed.
    ///
    /// `tab_id` is `Some` when emitted by a specific tab's selection model,
    /// and `None` when emitted programmatically with no tab context.
    SelectionChanged { tab_id: Option<u64> },
    /// Requests the paths selected in the active tab.
    ///
    /// Sent by the selection socket server, the update loop answers through
    /// `reply`.
    QuerySelection {
        reply: std::sync::mpsc::Sender<Vec<PathBuf>>,
    },
    /// Updates the default sorting method.
    SetDefaultSort(SortBy),
    #[allow(dead_code)]
    ToggleSingleClick,
    /// Toggle visibility of dotfiles and hidden items.
    ToggleHidden,
    /// Switch between grid card layout and compact list layout.
    ToggleListMode,
    /// Rotate through available sorting methods.
    CycleSort,
    /// Toggle between "Folders First" and mixed sorting.
    CycleFolderPriority,
    /// Adjust the icon size based on scroll delta.
    Zoom(f64),
    /// Toggles the directory listing between ascending and descending order.
    ToggleSortOrder,
    /// Sets sort direction: true for Ascending, false for Descending.
    SetAsc(bool),
}
