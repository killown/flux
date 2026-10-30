//! Navigation, history, recents and keyboard navigate mode.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum NavigationMsg {
    /// Set the maximum navigation history depth.
    SetMaxHistory(usize),
    /// Toggles the right location panel (Ctrl+L).
    ToggleLocationPanel,
    /// Cycle through sibling folders of the current directory whose name
    /// starts with `letter` (case-insensitive), navigating to each in turn.
    CycleByLetter(char),
    /// Cancel the cycle and, if we descended into a candidate, return to
    /// the anchor directory.
    CycleByLetterCancel,
    /// Re-anchor the cycle at the current directory. The next
    /// `CycleByLetter` builds a fresh candidate list from here.
    CycleByLetterReanchor,
    /// Turns Navigate mode on or off (`Ctrl+Insert`, or a click on the status-bar chip).
    ToggleNavigateMode,
    /// A plain printable key reached the grid. In Navigate mode it extends the
    /// prefix and jumps the selection, otherwise it is type-to-search as before.
    KeyChar(char),
    /// Enter on the grid: in Navigate mode, makes the folder just cycled into the
    /// new base for letters, otherwise activates the selection as before.
    KeyEnter,
    /// Backspace on the grid: shortens the Navigate prefix, or edits the search filter.
    KeyBackspace,
    /// Esc on the grid: leaves Navigate mode, or clears filters as before.
    KeyEscape,

    /// Sets the zero-based insertion index for the Recents row within the `[[sidebar]]` entry list.
    ///
    /// Values ≥ the number of configured sidebar entries place Recents after all of them.
    /// Has no effect when `show_recents` is `false`.
    SetRecentsRow(usize),
    /// Remove one or all entries from the recent-files list.
    ClearRecents,
    /// Result of `handle_clear_recents`: `Ok(())` means the XBEL file was
    /// rewritten, `Err` carries the I/O error message.
    ClearRecentsFinished(Result<(), String>),
    /// Update the current path and refresh the file list.
    Navigate(PathBuf),
    /// Navigate to a specific index in the recent folders stack.
    JumpToRecent(usize),
    /// Execute the primary action for all currently selected items.
    Activate,
    /// Move back in history.
    GoBack,
    /// Move forward in history.
    GoForward,
    /// Reload the current directory from disk.
    Refresh,
    /// Open all currently selected files or navigate to the selected directory.
    ///
    /// `position` is supplied by `GridView::connect_activate` and addresses the
    /// item directly without querying the selection model. `None` is used by
    /// keyboard shortcuts and falls back to `get_selection_with_meta()`.
    ///
    /// `tab_id` identifies the tab whose grid emitted the activation. It is
    /// `Some` for widget-driven activations and `None` for global shortcuts,
    /// which must operate on the currently active tab.
    Open {
        position: Option<u32>,
        tab_id: Option<u64>,
    },
    /// Activate explicit `(path, is_dir)` items, independent of the main grid's
    /// selection. Used by the preview folder grid for entries inside archives.
    ActivateItems(Vec<(PathBuf, bool)>),
    /// Toggles the "Recents" virtual entry in the sidebar.
    SetShowRecents(bool),
}
