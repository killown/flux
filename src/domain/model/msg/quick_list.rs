//! Quick Lists and the exclusive navigation list.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum QuickListMsg {
    /// Initiates a copy or move of selected items directly to a quick list destination.
    PerformQuickTransfer { dest: PathBuf, is_cut: bool },
    /// Files were dragged from the grid and dropped onto a quick-list tab button.
    MoveFilesToTarget {
        sources: Vec<PathBuf>,
        destination: PathBuf,
    },
    /// Add a directory to the exclusive navigation list.
    ///
    /// `Some(path)` targets an explicit path (e.g. from a context menu action).
    /// `None` resolves the path from the current selection or working directory,
    /// which is what the `add_to_quick_list` menu action does.
    AddExclusive(Option<PathBuf>),
    /// Add the folder shown in the grid to the exclusive navigation list,
    /// ignoring the selection (the `Insert` keyboard shortcut).
    AddCurrentFolderExclusive,
    /// Clear all items from the exclusive navigation list.
    ClearExclusive,
    /// Switch to the next directory in the exclusive list.
    NextExclusive,
    /// Switch to the previous directory in the exclusive list.
    PrevExclusive,
    /// Tab: go to the current Quick List slot, or advance when already in it.
    QuickListTab,
    /// Ctrl+1..9: jump to Quick List slot N (1-based).
    QuickListJump(usize),
    /// Redraw the Quick List panel from the current lists.
    ///
    /// Sent automatically after any mutation of the Quick Lists so the widget
    /// tree always reflects the current state. A no-op while the panel has
    /// never been opened.
    RebuildQuickPanel,
    /// Toggles the right Quick List panel (Ctrl+Shift+Q).
    ToggleQuickListPanel,
    /// Makes list `n` the one Tab and Ctrl+PageUp/PageDown walk.
    QuickListSelect(usize),
    /// Opens a folder from list `list`: selects the list, puts its slot on
    /// `path` and navigates there.
    QuickListOpen { list: usize, path: PathBuf },
    /// Creates an empty list. A blank or taken name is refused with a toast.
    QuickListNew(String),
    /// Renames a list. A blank or taken name is refused with a toast.
    QuickListRename { list: usize, name: String },
    /// Deletes a list. The last list cannot be deleted.
    QuickListDelete(usize),
    /// Empties a list but keeps it.
    QuickListClearList(usize),
    /// Adds the folder shown in the grid to the front of list `n`.
    QuickListAddCurrent(usize),
    /// Removes `path` from list `list`.
    QuickListRemove { list: usize, path: PathBuf },
    /// Moves `path` from list `from` to the front of list `to`.
    QuickListMove {
        path: PathBuf,
        from: usize,
        to: usize,
    },
    /// Points slot `slot` of list `list` at the folder shown in the grid.
    QuickListSetSlot { list: usize, slot: usize },
}
