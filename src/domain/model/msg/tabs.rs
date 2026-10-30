//! Tab lifecycle.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TabsMsg {
    /// Opens multiple directory paths into tabs in a single batch.
    OpenTabs(Vec<PathBuf>),
    /// Opens a new window tab with an optional initial directory path.
    NewTab(Option<PathBuf>),
    /// Closes a tab by index, or closes the currently active tab if `None`.
    CloseTab(Option<usize>),
    /// Switches keyboard focus and display to the tab at the given index.
    SwitchTab(usize),
    /// The user dragged a tab to a new position. `moved_id` is the stable
    /// `TabState.id` of the moved tab, `new_position` is its new visual
    /// position in the `adw::TabView`.
    TabReordered { moved_id: u64, new_position: usize },
    /// Cycles to the next tab in sequential order.
    NextTab,
    /// Cycles to the previous tab in reverse sequential order.
    PrevTab,
}
