//! Preview panel and pane focus.

use std::path::PathBuf;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PreviewMsg {
    /// Opens the preview panel in wide mode for CLI `--preview`.
    StartupPreview,
    /// Dispatches neighbour prefetch on selection change when the preview panel is open.
    PreviewPrefetch { path: PathBuf, session: u64 },
    /// Delivers a prefetched payload so the cache stays warm without the user selecting it.
    PreviewPrefetched {
        path: PathBuf,
        content: std::sync::Arc<crate::ui::panels::preview::PreviewContent>,
    },
    /// Toggle the peek-preview side panel. Bound to `Space` in the grid.
    TogglePreview,
    /// Load a preview for `path` into the preview panel. Dispatched by
    /// `TogglePreview` and, when the panel is already open, by
    /// `SelectionChanged`. `session` guards against stale background results.
    PreviewLoad { path: PathBuf, session: u64 },
    /// Deliver a resolved preview payload to the panel. Stale sessions dropped.
    PreviewReady {
        path: PathBuf,
        session: u64,
        content: crate::ui::panels::preview::PreviewContent,
    },
    /// Updates and persists the width of the peek-preview panel.
    SetPreviewPanelWidth(i32),
    /// Toggles full-screen preview: the grid, header bar and status bar are hidden, the window
    /// goes full screen and the preview fills it. Esc or the preview header button leave it.
    TogglePreviewFullscreen,
    /// Leaves full-screen preview if it is active (sent on Esc), no-op otherwise.
    ExitPreviewFullscreen,
    /// Moves keyboard focus between the main grid and the folder grid in the preview panel.
    TogglePreviewGridFocus,
    /// Moves keyboard focus one pane to the left: right-side panels, then the file grid,
    /// then the left sidebar.
    FocusPaneLeft,
    /// Moves keyboard focus one pane to the right: left sidebar, then the file grid,
    /// then the open right-side panels.
    FocusPaneRight,
}
