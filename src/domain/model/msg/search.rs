//! Search, filters and the header search state.

use crate::domain::model::{AdvancedSearchParams, ExtensionMatch};
use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SearchMsg {
    /// Opens the preview panel (if closed) and applies the selected item's MIME filter.
    TogglePreviewAndFilterBySelectedMime,
    /// Toggles an ephemeral filter showing only files sharing the selected item's MIME type.
    FilterBySelectedMime,
    /// Removes the ephemeral MIME filter and restores the full listing.
    ClearMimeFilter,
    /// Cycles the view filter: All -> OnlyFolders -> OnlyFiles -> All.
    CycleFilterMode,
    /// Toggles the background SQLite FTS5 file indexer.
    SetEnableFileIndexing(bool),
    /// Persists and updates the search panel width in pixels.
    SetSearchPanelWidth(i32),
    /// Toggles the visibility of the lazy-initialized right search sidebar panel.
    ToggleSearchPanel,
    /// Set the maximum search results limit.
    SetMaxSearchResults(usize),
    /// Sets the maximum number of content search results.
    /// Value is persisted in the config file.
    SetMaxContentSearchResults(usize),
    /// A batch of files matched a recursive extension/glob search.
    ExtensionSearchBatch {
        results: Vec<ExtensionMatch>,
        session: u64,
    },
    /// Kick off a recursive filename search using glob patterns.
    StartExtensionSearch(Vec<String>),
    /// Recursive search with full advanced constraints.
    StartAdvancedSearch(AdvancedSearchParams),
    /// Syncs the inline path entry widget text to the current directory.
    SyncPathEntry,
    /// Initiates a deep content search within the current directory.
    /// Syntax: `:term` to search all files, or `:.ext:term` to filter by extension
    /// (e.g., `:.rs:hello` for `.rs` files). Multiple extensions can be comma‑separated
    /// (e.g., `:.rs,py:hello`). `term` must be at least 3 characters.
    /// The second argument is the optional extension filter (without the dot).
    StartContentSearch(String, Option<String>),
    /// Cancels any in-flight content search.
    CancelContentSearch,
    /// Append a character to the active search filter.
    SearchInput(char),
    /// Remove the last character from the active search filter.
    SearchBackspace,
    /// Synchronize the search entry state with the model.
    CloseSearchSync,
    /// Toggle the glob-pattern filter bar in the header.
    ///
    /// Opened by the header filter button and by `Ctrl+Shift+F` while the
    /// button is hidden. Closing it clears any active extension filter and
    /// returns the header to the path view. A single toggle message keeps the
    /// button and the chord on one code path.
    ToggleFilterBar,
    /// Update the string used to filter the current view.
    UpdateFilter(String),
    /// Persist a new set of glob patterns as the session-scoped navigation filter.
    ///
    /// Patterns use shell glob syntax (`*`, `?`) and are matched case-insensitively
    /// against each file's full name (not just its extension), allowing patterns
    /// like `*.py`, `a*`, or `report??.pdf`. An empty `Vec` is treated the same
    /// as [`SearchMsg::ClearExtensionFilter`].
    SetExtensionFilter(Vec<String>),
    /// Removes all active session-scoped navigation filter patterns.
    ///
    /// Equivalent to `SetExtensionFilter(vec![])`. Directories are always visible
    /// regardless of filter state, clearing restores files to their unfiltered listing.
    ClearExtensionFilter,
    /// Signal that a thumbnail has been successfully generated.
    /// A file matched a content search query. Carries the path, the matching
    /// line text, and the session id for stale-result rejection.
    ContentSearchResult {
        path: PathBuf,
        line: String,
        line_number: usize,
        session: u64,
    },
    /// The content search walk finished (or was cancelled). Used to clear the
    /// loading indicator and select the first result.
    ContentSearchDone { session: u64 },
    /// Switch the header bar between path, entry, and search modes.
    SwitchHeader(String),
}
