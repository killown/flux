//! Typed groups of `FluxApp` session state that hold toolkit handles.

use std::path::PathBuf;

/// Session state of the peek-preview feature outside the panel itself: the prefetch cache,
/// the wide and full-screen modes, and inline video autoplay.
#[derive(Debug)]
pub struct PreviewState {
    /// Bounded LRU of prefetched preview payloads keyed on path + mtime.
    pub cache: std::sync::Arc<crate::app::update::view::prefetch::PreviewCache>,
    /// One-shot: open the preview panel in wide mode after the first folder load (CLI `--preview`).
    pub startup: bool,
    /// `Some(sidebar was visible)` while wide-preview mode is active, `None` otherwise.
    pub wide_sidebar: Option<bool>,
    /// `Some((wide mode was already on, window was already full screen))` while full-screen
    /// preview is active, `None` otherwise.
    pub fullscreen: Option<(bool, bool)>,
    /// Timestamps of recent autoplay launches, used to enforce the launch-rate
    /// limit that prevents GStreamer pipeline FD exhaustion under rapid selection.
    pub video_launches: std::collections::VecDeque<std::time::Instant>,
    /// Set when the launch-rate limit trips, autoplay is suppressed until this instant.
    pub video_cooldown_until: Option<std::time::Instant>,
    /// Path of the video currently playing inline in the selected card.
    pub active_video: Option<PathBuf>,
    /// Active GLib timeout source ID used to debounce rapid selection changes for video previews.
    pub video_source: Option<glib::SourceId>,
}

/// Content-search state and the layout it saves and restores.
#[derive(Debug)]
pub struct SearchState {
    /// Indicates if an active content search is currently running.
    pub content_searching: bool,
    /// Token to cancel the currently active content search operation.
    pub cancellable: Option<gio::Cancellable>,
    pub last_was_advanced: bool,
    pub saved_layout: bool,
    /// Maximum grid columns before content search, restored when search ends.
    pub saved_max_columns: u32,
    /// The user's preferred list‑mode state before a content search forced it to `true`.
    pub saved_list_mode: bool,
    /// Flag indicating the search interface was just initialized to trigger focus.
    pub just_opened: bool,
}

impl SearchState {
    /// Stashes the user's grid layout before a search forces list mode.
    pub fn stash_layout(&mut self, list_mode: bool, max_columns: u32) {
        if !self.saved_layout {
            self.saved_list_mode = list_mode;
            self.saved_max_columns = max_columns;
            self.saved_layout = true;
        }
    }

    /// Returns the layout to restore, `(list_mode, max_columns)`, and ends the
    /// stash session so the next search stashes afresh.
    pub fn take_layout(&mut self) -> (bool, u32) {
        self.saved_layout = false;
        (self.saved_list_mode, self.saved_max_columns)
    }
}

/// The bottom and full-window terminals.
#[derive(Debug)]
pub struct TerminalState {
    /// The Paned widget that contains the terminal.
    pub paned: Option<gtk::Paned>,
    /// Whether the terminal has been cleared on first open.
    pub cleared: bool,
    /// The embedded VTE terminal widget.
    pub widget: crate::infra::terminal::Terminal,
    /// Tracks whether the terminal's PTY shell process has been spawned.
    pub spawned: bool,
    /// Whether the terminal panel is currently visible.
    pub visible: bool,
    /// True while the full-window terminal mode is active. Only meaningful
    /// when `config.ui.terminal.position == TerminalPosition::Fullscreen`.
    pub fullscreen: bool,
    /// Dedicated terminal instance for full-window mode. Lazily created on first
    /// entry, never reparented into the paned or sidebar. Killed on window close.
    pub fullscreen_widget: Option<crate::infra::terminal::Terminal>,
    /// Handle to the container inside the `"terminal"` Stack page.
    pub fullscreen_box: Option<gtk::Box>,
    /// True once the fullscreen terminal has spawned its shell at least once.
    /// Independent from `spawned` (bottom pane) and the sidebar panel.
    pub fullscreen_spawned: bool,
}
