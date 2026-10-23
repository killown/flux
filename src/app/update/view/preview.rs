//! Peek-preview panel handlers. Toggled by `Ctrl+Space` from the file grid.
//!
//! `compute_preview` runs on a blocking worker and picks the richest viewer
//! for the file: PDF pages, full-resolution images, syntect-highlighted
//! text, Markdown, CSV/TSV tables, hex dumps, folder / archive listings, or
//! media tag cards. Every result carries a [`FileInfo`] for the Details
//! drawer.

use crate::model::{AppMsg, FluxApp, RightPanelType};
use crate::ui::panels::preview::{FileInfo, FolderEntry, PreviewContent};
use crate::ui::panels::preview_binary;
use crate::ui::panels::preview_markdown;
use crate::ui::panels::preview_office;
use crate::ui::panels::preview_registry;
use crate::ui::panels::preview_syntax;
use crate::ui::panels::preview_table;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

impl FluxApp {
    pub fn handle_toggle_preview(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_toggle_preview");
        if !self.preview_panel_visible {
            // Stop grid autoplay before the panel is shown. A live gtk::Video in
            // the grid reports its intrinsic size as natural width, and while the
            // preview panel is squeezing the grid's column allocation the toplevel
            // would otherwise negotiate grid_natural + panel_width and grow past
            // the monitor.
            self.stop_video_preview();
            self.toggle_sidebar_right_panel(RightPanelType::Preview, sender);
            if self.config.ui.wide_preview {
                self.enter_wide_preview();
            }

            // Nothing selected yet (e.g. opened from the header button): select
            // the first item so the panel opens populated. The resulting
            // SelectionChanged, now that the panel is open, loads its preview.
            // Deferred to idle so the panel/grid allocation settles first.
            let Some(path) = self.get_selected_path() else {
                let view = self.tabs[self.active_tab_index].files.view.clone();
                glib::idle_add_local_once(move || {
                    if let Some(model) = view
                        .model()
                        .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                    {
                        if model.n_items() > 0 {
                            model.select_item(0, true);
                        }
                    }
                });
                return;
            };

            let session = self.preview_session.fetch_add(1, Ordering::SeqCst) + 1;
            sender.input(AppMsg::PreviewLoad { path, session });
            return;
        }

        // Panel is already open: Space (or the header close button) closes it.
        // Selection changes are already synced by `handle_selection_changed`,
        // so no reload is needed here. Bumping the session discards any
        // in-flight preview results.
        self.preview_session.fetch_add(1, Ordering::SeqCst);

        // When the preview panel is closed, any active MIME filter is cleared.
        // This makes the "open preview and filter" action (Space) a true toggle.
        if self.mime_filter.is_some() {
            self.mime_filter = None;
            self.mime_filter_from_space = false; // Still good practice to reset
            crate::ui::panels::preview_registry::set_mime_filter_active(false);
            sender.input(AppMsg::Refresh);
        }

        self.stop_preview_media();
        let _ = preview_registry::with(|h| h.reset_mute());
        let _ = preview_registry::with(|h| h.release_folder_grid());
        self.exit_wide_preview();
        self.toggle_sidebar_right_panel(RightPanelType::Preview, sender);
    }

    /// Opens the preview panel if closed, then applies the selected item's MIME filter.
    ///
    /// When nothing is selected, falls back to the first item so the shortcut
    /// always produces a preview + filter, never a silent no-op.
    pub fn handle_toggle_preview_and_filter_by_selected_mime(
        &mut self,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_toggle_preview_and_filter_by_selected_mime");
        if !self.preview_panel_visible {
            self.handle_toggle_preview(sender);
        }

        if self.mime_filter.is_some() {
            return;
        }

        self.mime_filter_from_space = true;

        if self.get_selected_path().is_none() {
            let view = self.tabs[self.active_tab_index].files.view.clone();
            let sender_clone = sender.clone();
            glib::idle_add_local_once(move || {
                let Some(model) = view
                    .model()
                    .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                else {
                    return;
                };
                if model.n_items() == 0 {
                    return;
                }
                model.select_item(0, true);
                sender_clone.input(AppMsg::FilterBySelectedMime);
            });
            return;
        }

        sender.input(AppMsg::FilterBySelectedMime);
    }

    /// Toggles wide-preview mode (sidebar hidden, panel fills the remaining width).
    /// Also persists `config.ui.wide_preview` so the next open matches.
    pub fn handle_toggle_wide_preview(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_toggle_wide_preview");
        if self.wide_preview_sidebar.is_some() {
            self.set_wide_preview_default(false);
            self.exit_wide_preview();
            return;
        }

        if !self.preview_panel_visible {
            self.handle_toggle_preview(sender);
        }
        self.set_wide_preview_default(true);
        self.enter_wide_preview();
    }

    /// Opens the preview panel in wide mode for CLI `--preview`, without touching the saved wide setting.
    ///
    /// Runs right after the window is built, without waiting for the folder to load.
    /// `startup_preview` stays set until the first item exists and can be selected
    /// (see [`Self::schedule_startup_preview_selection`]).
    pub fn handle_startup_preview(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_startup_preview");
        if !self.preview_panel_visible {
            self.handle_toggle_preview(sender);
        }
        self.enter_wide_preview();
        // The folder may already have delivered items before this message ran.
        self.schedule_startup_preview_selection();
    }

    /// Selects the first item once, so the already-open `--preview` panel gets populated.
    /// No-op until the panel is open and the grid has items, clears `startup_preview` when it fires.
    pub fn schedule_startup_preview_selection(&mut self) {
        if !self.startup_preview || !self.preview_panel_visible {
            return;
        }
        let view = self.tabs[self.active_tab_index].files.view.clone();
        let has_items = view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
            .is_some_and(|m| m.n_items() > 0);
        if !has_items {
            return;
        }
        self.startup_preview = false;
        // Deferred to idle so the grid/panel allocation settles first.
        glib::idle_add_local_once(move || {
            if let Some(model) = view
                .model()
                .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
            {
                if model.n_items() > 0 {
                    model.select_item(0, true);
                }
            }
        });
    }

    /// Stops the preview panel's inline media (video via `gtk::Video`, audio via
    /// the `mpv` child) and the grid card's autoplay preview. Safe to call when
    /// nothing is playing.
    pub fn stop_preview_media(&mut self) {
        let _ = preview_registry::with(|h| h.stop_media());
        self.stop_video_preview();
    }

    /// Header-button handler: stores the permanent wide setting and applies it right away.
    pub fn handle_set_wide_preview_default(&mut self, on: bool) {
        crate::hit!("handle_set_wide_preview_default");
        if self.config.ui.wide_preview == on {
            return;
        }
        self.set_wide_preview_default(on);
        if on {
            self.enter_wide_preview();
        } else {
            self.exit_wide_preview();
        }
    }

    /// Saves `config.ui.wide_preview` and syncs the panel's header button.
    fn set_wide_preview_default(&mut self, on: bool) {
        if self.config.ui.wide_preview != on {
            self.config.ui.wide_preview = on;
            crate::utils::save_config(&self.config);
        }
        let _ = preview_registry::with(|h| h.set_wide_default_active(on));
    }

    /// Enters wide-preview mode when the panel is open, no-op if already wide.
    fn enter_wide_preview(&mut self) {
        if self.wide_preview_sidebar.is_some() || !self.preview_panel_visible {
            return;
        }
        let Some(revealer) = self.preview_panel_revealer.clone() else {
            return;
        };

        self.wide_preview_sidebar = Some(self.sidebar_visible);
        self.apply_sidebar_visibility(false);
        set_wide_preview_layout(
            &revealer,
            true,
            self.is_list_mode,
            self.config.ui.preview_panel_width,
        );
    }

    /// Toggles full-screen preview: the main column (header bar, tab bar, file grid, status bar)
    /// is hidden, the window goes full screen and the preview panel fills it.
    pub fn handle_toggle_preview_fullscreen(&mut self) {
        crate::hit!("handle_toggle_preview_fullscreen");
        use gtk::prelude::*;

        if self.preview_fullscreen.is_some() {
            self.leave_preview_fullscreen(true);
            return;
        }
        if !self.preview_panel_visible {
            return;
        }
        let Some(revealer) = self.preview_panel_revealer.clone() else {
            return;
        };

        let was_wide = self.wide_preview_sidebar.is_some();
        self.enter_wide_preview();

        let window = revealer.root().and_downcast::<gtk::Window>();
        let window_was_fullscreen = window.as_ref().is_some_and(|w| w.is_fullscreen());
        set_main_column_visible(&revealer, false);
        if let Some(w) = window.as_ref() {
            w.fullscreen();
        }
        self.preview_fullscreen = Some((was_wide, window_was_fullscreen));
        let _ = preview_registry::with(|h| h.set_fullscreen_active(true));
    }

    /// Esc: leaves full-screen preview when active.
    pub fn handle_exit_preview_fullscreen(&mut self) {
        self.leave_preview_fullscreen(true);
    }

    /// Moves keyboard focus between the main grid and the folder grid in the preview panel.
    pub fn handle_toggle_preview_grid_focus(&mut self) {
        crate::hit!("handle_toggle_preview_grid_focus");
        let in_folder_grid = preview_registry::with(|h| h.folder_grid_has_focus()).unwrap_or(false);

        if in_folder_grid {
            // Return to the main grid.
            if let Some(tab) = self.tabs.get(self.active_tab_index) {
                tab.files.view.grab_focus();
            }
            return;
        }

        // Not in the folder grid: try to enter it if possible, otherwise stay on the grid.
        if !self.exclusive_list.is_empty() || !self.preview_panel_visible {
            return;
        }
        let entered = preview_registry::with(|h| h.focus_folder_grid()).unwrap_or(false);
        let _ = entered; // focus_folder_grid should return bool
    }

    /// Moves keyboard focus one pane left or right across the sidebar, the file grid
    /// and the open right-side panels, in the order they appear in the window.
    ///
    /// Does nothing at either end. Hidden sidebar and collapsed panels are skipped, as
    /// are panes that refuse focus.
    ///
    /// # Arguments
    ///
    /// * `step` - `-1` to move left, `1` to move right.
    pub fn handle_focus_pane(&mut self, step: isize) {
        crate::hit!("handle_focus_pane");
        let Some(grid) = self
            .tabs
            .get(self.active_tab_index)
            .map(|tab| tab.files.view.clone().upcast::<gtk::Widget>())
        else {
            return;
        };

        // (container used to locate focus, widget that receives focus)
        let mut panes: Vec<(gtk::Widget, gtk::Widget)> = Vec::new();
        if self.sidebar_visible {
            if let Some(container) = self.sidebar_widget.clone() {
                panes.push((container, self.sidebar.widget().clone().upcast()));
            }
        }
        let grid_idx = panes.len();
        panes.push((grid.clone(), grid.clone()));

        let mut preview_idx = None;
        let right_panels = [
            (&self.search_panel_revealer, false),
            (&self.tag_panel_revealer, false),
            (&self.diff_panel_revealer, false),
            (&self.location_panel_revealer, false),
            (&self.connect_panel_revealer, false),
            (&self.preview_panel_revealer, true),
            (&self.actions_panel_revealer, false),
            (&self.terminal_panel_revealer, false),
        ];
        for (revealer, is_preview) in right_panels {
            let Some(revealer) = revealer.as_ref().filter(|r| r.reveals_child()) else {
                continue;
            };
            let widget = revealer.clone().upcast::<gtk::Widget>();
            panes.push((widget.clone(), widget));
            if is_preview {
                preview_idx = Some(panes.len() - 1);
            }
        }

        let focused = grid.root().and_then(|root| root.focus());
        let current = focused
            .as_ref()
            .and_then(|f| panes.iter().position(|(c, _)| f == c || f.is_ancestor(c)))
            .unwrap_or(grid_idx);

        let len = panes.len() as isize;
        let mut idx = current as isize;
        loop {
            idx += step;
            if idx < 0 || idx >= len {
                return;
            }
            let i = idx as usize;
            let (_, target) = &panes[i];
            let focused_ok = if i == grid_idx {
                target.grab_focus()
            } else if Some(i) == preview_idx
                && preview_registry::with(|h| h.focus_folder_grid()).unwrap_or(false)
            {
                true
            } else {
                target.child_focus(gtk::DirectionType::TabForward)
            };
            if focused_ok {
                return;
            }
        }
    }

    /// Restores the main column and the window state, no-op when full-screen preview is off.
    ///
    /// `restore_wide` also drops the wide layout if full-screen had to turn it on, callers that
    /// are leaving wide mode themselves pass `false`.
    pub(crate) fn leave_preview_fullscreen(&mut self, restore_wide: bool) {
        use gtk::prelude::*;

        let Some((was_wide, window_was_fullscreen)) = self.preview_fullscreen.take() else {
            return;
        };
        if let Some(revealer) = self.preview_panel_revealer.as_ref() {
            set_main_column_visible(revealer, true);
            if !window_was_fullscreen {
                if let Some(w) = revealer.root().and_downcast::<gtk::Window>() {
                    w.unfullscreen();
                }
            }
        }
        let _ = preview_registry::with(|h| h.set_fullscreen_active(false));
        if restore_wide && !was_wide {
            self.exit_wide_preview();
        }
    }

    /// Re-applies the wide-preview split after a view-mode change.
    pub fn refresh_wide_preview_layout(&self) {
        if self.wide_preview_sidebar.is_none() {
            return;
        }
        if let Some(revealer) = self.preview_panel_revealer.as_ref() {
            set_wide_preview_layout(
                revealer,
                true,
                self.is_list_mode,
                self.config.ui.preview_panel_width,
            );
        }
    }

    /// Leaves wide-preview mode, restoring the sidebar and the normal panel layout, no-op when inactive.
    pub fn exit_wide_preview(&mut self) {
        // Leaving wide mode also ends full-screen preview, otherwise the grid would stay hidden.
        self.leave_preview_fullscreen(false);
        let Some(sidebar_was_visible) = self.wide_preview_sidebar.take() else {
            return;
        };
        self.apply_sidebar_visibility(sidebar_was_visible);
        if let Some(revealer) = self.preview_panel_revealer.clone() {
            set_wide_preview_layout(&revealer, false, false, self.config.ui.preview_panel_width);
        }
    }

    /// Shows or hides the sidebar for this session only, without touching the saved config.
    fn apply_sidebar_visibility(&mut self, visible: bool) {
        use gtk::prelude::*;
        self.sidebar_visible = visible;
        if let Some(ref widget) = self.sidebar_widget {
            widget.set_visible(visible);
        }
    }

    pub fn handle_preview_load(&self, path: PathBuf, session: u64) {
        crate::hit!("handle_preview_load");
        if session != self.preview_session.load(Ordering::SeqCst) {
            return;
        }

        if let Some(handle) = preview_registry::with(Clone::clone) {
            handle.apply(&PreviewContent::Loading);
        }

        let session_arc = self.preview_session.clone();
        let sender = crate::model::SENDER.get().cloned();
        relm4::spawn_blocking(move || {
            if session_arc.load(Ordering::SeqCst) != session {
                return;
            }
            let content = compute_preview_guarded(&path);
            if session_arc.load(Ordering::SeqCst) != session {
                return;
            }
            if let Some(s) = sender {
                let _ = s.send(AppMsg::PreviewReady {
                    path,
                    session,
                    content,
                });
            }
        });
    }

    pub fn handle_preview_ready(&self, path: PathBuf, session: u64, content: PreviewContent) {
        crate::hit!("handle_preview_ready");
        if session != self.preview_session.load(Ordering::SeqCst) {
            return;
        }
        if !self.preview_panel_visible {
            return;
        }
        if let Some(handle) = preview_registry::with(Clone::clone) {
            // `apply` runs on the GTK main thread. Log anything that blocks
            // the UI so offenders are visible instead of guessed at.
            let t = std::time::Instant::now();
            handle.apply(&content);
            handle.set_mime_filter_active(self.mime_filter.is_some());
            let ms = t.elapsed().as_millis();
            if ms > 30 {
                eprintln!(
                    "[flux][preview] apply blocked UI {ms} ms: {}",
                    path.display()
                );
            }
        }
    }

    pub fn handle_set_preview_panel_width(&mut self, val: i32) {
        crate::hit!("handle_set_preview_panel_width");
        let clamped = val.clamp(300, 2400);
        self.config.ui.preview_panel_width = clamped;
        crate::utils::save_config(&self.config);
    }
}

// ─── Limits ───────────────────────────────────────────────────────────────────

const TEXT_HEAD_BYTES: usize = 64 * 1024;
const TEXT_HEAD_LINES: usize = 500;
const HEX_BYTES: usize = 512;
const IMAGE_MAX_DIM: i32 = 2560;
const LISTING_SCAN_CAP: usize = 20_000;
/// Most entries the folder grid in the preview panel shows.
const FOLDER_GRID_MAX_ITEMS: usize = 2000;
/// Longest line handed to the text view. GTK/Pango lay out huge single
/// lines very slowly, so anything longer is cut with an ellipsis.
const MAX_LINE_CHARS: usize = 2000;
/// More highlight ranges than this are dropped (plain text is shown).
const MAX_HIGHLIGHT_RANGES: usize = 5000;
/// Hard wall-clock limit for computing one preview.
const PREVIEW_DEADLINE: Duration = Duration::from_secs(10);

const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "jpe", "png", "gif", "webp", "bmp", "tif", "tiff", "svg", "ico", "avif", "heic",
    "heif", "jxl", "xpm", "pnm", "tga", "icns",
];
const VIDEO_EXTS: &[&str] = &[
    "mp4", "mkv", "webm", "avi", "mov", "flv", "wmv", "m4v", "mpg", "mpeg", "ts", "ogv",
];
const AUDIO_EXTS: &[&str] = &["mp3", "flac", "m4a", "ogg", "wav", "aac", "wma", "opus"];
const TABLE_EXTS: &[&str] = &["csv", "tsv", "tab"];
const MARKDOWN_EXTS: &[&str] = &["md", "markdown", "mdown", "mkd", "mkdn"];

// ─── Entry point ──────────────────────────────────────────────────────────────

/// Runs [`compute_preview`] under a deadline. Never blocks the caller longer
/// than `PREVIEW_DEADLINE` and turns worker panics into a visible card, so
/// the panel always gets something to show instead of spinning forever.
pub(crate) fn compute_preview_guarded(path: &Path) -> PreviewContent {
    crate::hit!("compute_preview_guarded");
    use crate::i18n::tr;

    let (tx, rx) = std::sync::mpsc::channel();
    let owned = path.to_path_buf();
    let t0 = std::time::Instant::now();
    let spawned = std::thread::Builder::new()
        .name("flux-preview".into())
        .spawn(move || {
            let _ = tx.send(compute_preview(&owned));
        });
    if spawned.is_err() {
        return degraded(path, tr("Preview failed."));
    }

    match rx.recv_timeout(PREVIEW_DEADLINE) {
        Ok(content) => {
            let ms = t0.elapsed().as_millis();
            if ms > 200 {
                eprintln!("[flux][preview] compute took {ms} ms: {}", path.display());
            }
            content
        }
        Err(RecvTimeoutError::Timeout) => {
            eprintln!("[flux][preview] timed out: {}", path.display());
            degraded(path, tr("Preview timed out. Use Open to view this file."))
        }
        Err(RecvTimeoutError::Disconnected) => {
            eprintln!("[flux][preview] worker panicked: {}", path.display());
            degraded(path, tr("Preview failed. Use Open to view this file."))
        }
    }
}

/// Metadata card that touches no filesystem state, so it cannot hang itself.
fn degraded(path: &Path, note: String) -> PreviewContent {
    use crate::i18n::tr;

    let mut info = FileInfo {
        path: path.to_path_buf(),
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        ..Default::default()
    };
    info.extra.push((tr("Note"), note));
    PreviewContent::Metadata { info }
}

pub(crate) fn compute_preview(path: &Path) -> PreviewContent {
    crate::hit!("compute_preview");
    use crate::i18n::tr;

    let virtual_path = path
        .to_string_lossy()
        .starts_with(crate::services::archive::ARCHIVE_URI);
    if !virtual_path && !path.exists() {
        return PreviewContent::Error(tr("File no longer exists."));
    }

    let mut info = build_info(path, virtual_path);

    // For virtual archive paths, extract the entry once to the persistent
    // preview cache and run the normal preview pipeline against that real
    // path. This makes text, Markdown, CSV, PDF, Office, font and image
    // previews all work inside archives, not just audio/video.
    let real_path: Option<PathBuf> = if virtual_path {
        if let Some((archive_path, inner)) =
            crate::services::archive::parse_archive_uri(&path.to_string_lossy())
        {
            let extracted =
                crate::services::archive::extract_archive_entry_for_playback(&archive_path, &inner);
            if extracted.is_some() {
                info.extra.push((
                    tr("Extracted from"),
                    archive_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                ));
            }
            extracted
        } else {
            None
        }
    } else {
        Some(path.to_path_buf())
    };

    let Some(real_path) = real_path else {
        // Extraction failed: keep the metadata card so the panel is never blank.
        return PreviewContent::Metadata { info };
    };
    let path = real_path.as_path();

    if path.is_dir() {
        return dir_listing(path, info);
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let ext = ext.as_str();

    // ── PDF ─────────────────────────────────────────────────────────────────
    if ext == "pdf" {
        return pdf_preview(path, info);
    }

    // ── Office: convert to PDF, then render that PDF ────────────────────────
    if preview_office::OFFICE_EXTS.contains(&ext) && preview_office::soffice_binary().is_some() {
        if let Some(pdf_path) = preview_office::convert_to_pdf(path) {
            info.extra.push((
                tr("Converted from"),
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ));
            return pdf_preview(&pdf_path, info);
        }
        // Conversion failed, fall through to Metadata at the end.
    }

    // ── Executables: icon + embedded metadata ───────────────────────────────
    if let Some(icon) = preview_binary::inspect(path, ext, &mut info) {
        return match icon {
            Some(tex) => PreviewContent::Image { tex, info },
            None => PreviewContent::Metadata { info },
        };
    }

    // ── Archives ────────────────────────────────────────────────────────────
    if crate::services::archive::is_supported_archive(path) {
        if let Some(listing) = archive_listing(path, &mut info) {
            return listing;
        }
    }

    // ── CSV / TSV ───────────────────────────────────────────────────────────
    if TABLE_EXTS.contains(&ext) {
        if let Some(data) = preview_table::parse(path, ext) {
            info.extra.push((tr("Rows"), data.rows.len().to_string()));
            info.extra
                .push((tr("Columns"), data.headers.len().to_string()));
            let delim_name = if data.delimiter == b'\t' {
                "Tab"
            } else {
                "Comma"
            };
            info.extra.push((tr("Delimiter"), delim_name.to_string()));
            return PreviewContent::Table { data, info };
        }
        // Empty or unparseable, fall through.
    }

    // ── Images ──────────────────────────────────────────────────────────────
    if IMAGE_EXTS.contains(&ext) || info.mime.starts_with("image/") {
        if let Some((tex, extra)) = load_image_full(path) {
            info.extra.extend(extra);
            return PreviewContent::Image { tex, info };
        }
    }

    // ── Audio ───────────────────────────────────────────────────────────────
    if AUDIO_EXTS.contains(&ext) || info.mime.starts_with("audio/") {
        info.extra.extend(audio_info(path));
        let cover = crate::utils::media::generate_thumbnail_sync(path);
        return PreviewContent::Media {
            path: path.to_path_buf(),
            video: false,
            cover,
            info,
        };
    }

    // ── Video ───────────────────────────────────────────────────────────────
    if VIDEO_EXTS.contains(&ext) || info.mime.starts_with("video/") {
        info.extra.extend(video_info(path));
        return PreviewContent::Media {
            path: path.to_path_buf(),
            video: true,
            cover: None,
            info,
        };
    }

    // ── Fonts ───────────────────────────────────────────────────────────────
    let is_font = matches!(ext, "ttf" | "otf" | "woff" | "woff2" | "ttc")
        || info.mime.starts_with("font/")
        || info.mime == "application/x-font-ttf"
        || info.mime == "application/x-font-otf"
        || info.mime == "application/font-sfnt"
        || info.mime == "application/font-woff";
    if is_font {
        // The path is already a real on-disk file (extracted from the archive
        // above if this was a virtual path), so fontconfig can open it directly.
        if let Some(tex) = crate::utils::media::font_specimen(path, 1024) {
            return PreviewContent::Image { tex, info };
        }
    }

    // ── Thumbnailer (fonts, executables, RAW) ───────────────────────────────
    if let Some(tex) = crate::utils::media::generate_thumbnail_sync(path) {
        return PreviewContent::Image { tex, info };
    }

    // ── Text / Markdown / hex ───────────────────────────────────────────────
    let Ok(head) = read_head(path, TEXT_HEAD_BYTES) else {
        return PreviewContent::Metadata { info };
    };
    let hit_cap = head.len() >= TEXT_HEAD_BYTES;
    let probe = &head[..head.len().min(8192)];
    let texty_mime = is_texty_mime(&info.mime);

    if !probe.contains(&0) {
        if let Some((text, lossy)) = decode_head(&head, hit_cap, texty_mime) {
            let total_lines = text.lines().count();
            let mut clamped = false;
            let shown: String = text
                .lines()
                .take(TEXT_HEAD_LINES)
                .map(|l| match l.char_indices().nth(MAX_LINE_CHARS) {
                    Some((i, _)) => {
                        clamped = true;
                        format!("{}…", &l[..i])
                    }
                    None => l.to_string(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            let truncated = total_lines > TEXT_HEAD_LINES || hit_cap || clamped;

            info.extra.push((
                tr("Encoding"),
                if lossy {
                    "UTF-8 (lossy)".to_string()
                } else {
                    "UTF-8".to_string()
                },
            ));
            info.extra.push((
                tr("Line endings"),
                if text.contains("\r\n") { "CRLF" } else { "LF" }.to_string(),
            ));
            info.extra.push((
                tr("Lines"),
                if truncated {
                    format!("{total_lines}+")
                } else {
                    total_lines.to_string()
                },
            ));
            if !truncated {
                info.extra
                    .push((tr("Words"), text.split_whitespace().count().to_string()));
            }

            // Markdown takes priority over syntect for `.md` files.
            if MARKDOWN_EXTS.contains(&ext) && !truncated {
                let md = preview_markdown::render(&shown);
                return PreviewContent::Markdown {
                    rendered: md.text,
                    styles: md.styles,
                    source: shown,
                    info,
                };
            }

            let cfg_on = crate::utils::load_config().ui.preview_syntax_highlighting;
            // Skip syntect for clamped (very long line) input, and drop the
            // result if it would create an unreasonable number of tag ranges.
            let (mut highlighted, language) = if cfg_on && !ext.is_empty() && !clamped {
                preview_syntax::highlight(&shown, ext)
            } else {
                (Vec::new(), None)
            };
            if highlighted.len() > MAX_HIGHLIGHT_RANGES {
                highlighted.clear();
            }
            if let Some(ref lang) = language {
                info.extra.push((tr("Language"), lang.clone()));
            }

            return PreviewContent::Text {
                head: shown,
                truncated,
                hex: false,
                info,
                highlighted,
                language,
            };
        }
    }

    // Binary hex fallback.
    let n = head.len().min(HEX_BYTES);
    PreviewContent::Text {
        head: hex_dump(&head[..n]),
        truncated: info.size as usize > n,
        hex: true,
        info,
        highlighted: Vec::new(),
        language: None,
    }
}

/// Shared PDF path used both for real `.pdf` files and for office documents
/// after soffice conversion.
fn pdf_preview(path: &Path, mut info: FileInfo) -> PreviewContent {
    use crate::i18n::tr;
    match pdf_summary(path) {
        Some((pages, w_pt, h_pt)) => {
            info.extra.push((tr("Pages"), pages.to_string()));
            info.extra.push((
                tr("Page size"),
                format!("{:.0} × {:.0} mm", w_pt / 72.0 * 25.4, h_pt / 72.0 * 25.4),
            ));
            PreviewContent::Pdf {
                pages,
                info,
                render_path: path.to_path_buf(),
            }
        }
        None => {
            info.extra.push((
                tr("Note"),
                tr("Could not open this PDF (encrypted or damaged)."),
            ));
            PreviewContent::Metadata { info }
        }
    }
}

// ─── Info ─────────────────────────────────────────────────────────────────────

fn build_info(path: &Path, virtual_path: bool) -> FileInfo {
    crate::hit!("build_info");
    use crate::i18n::tr;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut info = FileInfo {
        path: path.to_path_buf(),
        name,
        ..Default::default()
    };

    if virtual_path {
        info.mime = crate::utils::media::get_mime_type(path);
        return info;
    }

    let is_dir = path.is_dir();
    info.mime = if is_dir {
        "inode/directory".to_string()
    } else {
        crate::utils::media::get_mime_type(path)
    };

    if let Ok(meta) = std::fs::metadata(path) {
        info.size = if is_dir { 0 } else { meta.len() };
        info.mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode() & 0o7777;
        info.extra.push((
            tr("Permissions"),
            format!("{} ({:o})", mode_string(mode), mode),
        ));
    }

    if let Ok(target) = std::fs::read_link(path) {
        info.extra
            .push((tr("Symlink to"), target.to_string_lossy().into_owned()));
    }
    info
}

fn mode_string(mode: u32) -> String {
    let mut s = String::with_capacity(9);
    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        s.push(if bits & 4 != 0 { 'r' } else { '-' });
        s.push(if bits & 2 != 0 { 'w' } else { '-' });
        s.push(if bits & 1 != 0 { 'x' } else { '-' });
    }
    s
}

// ─── PDF ──────────────────────────────────────────────────────────────────────

fn pdf_summary(path: &Path) -> Option<(usize, f64, f64)> {
    crate::hit!("pdf_summary");
    let doc = poppler::PopplerDocument::new_from_file(path, None).ok()?;
    let pages = doc.get_n_pages();
    if pages == 0 {
        return None;
    }
    let (w, h) = doc.get_page(0)?.get_size();
    Some((pages, w, h))
}

// ─── Images ───────────────────────────────────────────────────────────────────

fn load_image_full(path: &Path) -> Option<(gtk::gdk::Texture, Vec<(String, String)>)> {
    crate::hit!("load_image_full");
    use crate::i18n::tr;

    let (format, w, h) = gdk_pixbuf::Pixbuf::file_info(path)?;
    let pixbuf = if w.max(h) > IMAGE_MAX_DIM {
        gdk_pixbuf::Pixbuf::from_file_at_scale(path, IMAGE_MAX_DIM, IMAGE_MAX_DIM, true).ok()?
    } else {
        gdk_pixbuf::Pixbuf::from_file(path).ok()?
    };
    let pixbuf = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);

    let extra = vec![
        (tr("Dimensions"), format!("{w} × {h} px")),
        (
            tr("Format"),
            format.name().map(|n| n.to_uppercase()).unwrap_or_default(),
        ),
    ];
    Some((gtk::gdk::Texture::for_pixbuf(&pixbuf), extra))
}

// ─── Audio / video ────────────────────────────────────────────────────────────

fn audio_info(path: &Path) -> Vec<(String, String)> {
    crate::hit!("audio_info");
    use crate::i18n::tr;
    use lofty::prelude::*;
    use lofty::probe::Probe;

    let mut rows = Vec::new();
    let Some(tagged) = Probe::open(path).ok().and_then(|p| p.read().ok()) else {
        return rows;
    };

    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        if let Some(v) = tag.title() {
            rows.push((tr("Title"), v.to_string()));
        }
        if let Some(v) = tag.artist() {
            rows.push((tr("Artist"), v.to_string()));
        }
        if let Some(v) = tag.album() {
            rows.push((tr("Album"), v.to_string()));
        }
        if let Some(v) = tag.genre() {
            rows.push((tr("Genre"), v.to_string()));
        }
        if let Some(v) = tag
            .get_string(lofty::tag::ItemKey::Year)
            .or_else(|| tag.get_string(lofty::tag::ItemKey::RecordingDate))
        {
            rows.push((tr("Year"), v.to_string()));
        }
    }

    let props = tagged.properties();
    rows.push((tr("Duration"), fmt_secs(props.duration().as_secs_f64())));
    if let Some(b) = props.audio_bitrate() {
        rows.push((tr("Bitrate"), format!("{b} kbps")));
    }
    if let Some(sr) = props.sample_rate() {
        rows.push((tr("Sample rate"), format!("{sr} Hz")));
    }
    if let Some(ch) = props.channels() {
        rows.push((tr("Channels"), ch.to_string()));
    }
    rows
}

fn video_info(path: &Path) -> Vec<(String, String)> {
    crate::hit!("video_info");
    use crate::i18n::tr;
    use std::collections::HashMap;

    let mut rows = Vec::new();
    let Ok(out) = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height,r_frame_rate:format=duration,bit_rate",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(path)
        .output()
    else {
        return rows;
    };
    if !out.status.success() {
        return rows;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let kv: HashMap<&str, &str> = text.lines().filter_map(|l| l.split_once('=')).collect();

    if let (Some(w), Some(h)) = (kv.get("width"), kv.get("height")) {
        rows.push((tr("Resolution"), format!("{w} × {h}")));
    }
    if let Some(c) = kv.get("codec_name") {
        rows.push((tr("Codec"), c.to_uppercase()));
    }
    if let Some(fps) = kv.get("r_frame_rate").and_then(|r| {
        let (n, d) = r.split_once('/')?;
        let (n, d): (f64, f64) = (n.parse().ok()?, d.parse().ok()?);
        (d > 0.0).then_some(n / d)
    }) {
        rows.push((tr("Frame rate"), format!("{fps:.2} fps")));
    }
    if let Some(d) = kv.get("duration").and_then(|d| d.parse::<f64>().ok()) {
        rows.push((tr("Duration"), fmt_secs(d)));
    }
    if let Some(b) = kv.get("bit_rate").and_then(|b| b.parse::<f64>().ok()) {
        rows.push((tr("Bitrate"), format!("{:.0} kbps", b / 1000.0)));
    }
    rows
}

fn fmt_secs(secs: f64) -> String {
    let t = secs.max(0.0).round() as u64;
    let (h, m, s) = (t / 3600, (t % 3600) / 60, t % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

// ─── Folders / archives ───────────────────────────────────────────────────────

fn dir_listing(path: &Path, mut info: FileInfo) -> PreviewContent {
    crate::hit!("dir_listing");
    use crate::i18n::tr;

    let Ok(rd) = std::fs::read_dir(path) else {
        return PreviewContent::Error(tr("Cannot read this folder."));
    };

    let show_hidden = crate::utils::load_config().ui.show_hidden_by_default;
    let mut entries: Vec<(String, bool, PathBuf)> = Vec::new();
    let mut scanned = 0usize;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        scanned += 1;
        if entries.len() < LISTING_SCAN_CAP {
            let is_dir = e
                .file_type()
                .is_ok_and(|t| t.is_dir() || (t.is_symlink() && e.path().is_dir()));
            entries.push((name, is_dir, e.path()));
        }
    }
    let dirs = entries.iter().filter(|e| e.1).count();
    let capped = scanned > entries.len();

    info.extra.push((
        tr("Items"),
        if capped {
            format!("{scanned}+")
        } else {
            scanned.to_string()
        },
    ));
    info.extra.push((tr("Folders"), dirs.to_string()));
    info.extra
        .push((tr("Files"), (entries.len() - dirs).to_string()));

    // Folders first, then by name. The lowercase key is built once per entry, not per comparison.
    entries.sort_by_cached_key(|e| (!e.1, e.0.to_lowercase()));

    let more = entries.len().saturating_sub(FOLDER_GRID_MAX_ITEMS);
    let entries = entries
        .into_iter()
        .take(FOLDER_GRID_MAX_ITEMS)
        .map(|(name, is_dir, path)| {
            let meta = std::fs::metadata(&path).ok();
            let size = if is_dir {
                0
            } else {
                meta.as_ref().map_or(0, |m| m.len())
            };
            let mtime = meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64);
            FolderEntry {
                name,
                path,
                is_dir,
                size,
                mtime,
            }
        })
        .collect();

    PreviewContent::Folder {
        entries,
        more,
        info,
    }
}

fn archive_listing(path: &Path, info: &mut FileInfo) -> Option<PreviewContent> {
    crate::hit!("archive_listing");
    use crate::i18n::tr;

    let mut entries = crate::services::archive::list_archive_entries(path, "", None).ok()?;
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    info.extra
        .push((tr("Top-level entries"), entries.len().to_string()));

    // Archives reuse the folder grid: each entry becomes a cell addressed by its
    // `archive://` URI, so thumbnails and activation take the same paths as a folder.
    let more = entries.len().saturating_sub(FOLDER_GRID_MAX_ITEMS);
    let entries = entries
        .into_iter()
        .take(FOLDER_GRID_MAX_ITEMS)
        .map(|e| FolderEntry {
            path: crate::services::archive::build_archive_uri(path, &e.inner_path),
            size: if e.is_dir { e.child_count } else { e.size },
            name: e.name,
            is_dir: e.is_dir,
            mtime: e.mtime,
        })
        .collect();

    Some(PreviewContent::Folder {
        entries,
        more,
        info: info.clone(),
    })
}

// ─── Text / hex ───────────────────────────────────────────────────────────────

fn is_texty_mime(mime: &str) -> bool {
    mime.starts_with("text/")
        || mime.ends_with("+xml")
        || mime.ends_with("+json")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-yaml"
                | "application/yaml"
                | "application/toml"
                | "application/sql"
                | "application/x-shellscript"
                | "application/x-sh"
                | "application/x-desktop"
                | "application/x-perl"
                | "application/x-php"
                | "application/x-ruby"
                | "application/x-subrip"
        )
}

fn decode_head(bytes: &[u8], hit_cap: bool, texty_mime: bool) -> Option<(String, bool)> {
    match std::str::from_utf8(bytes) {
        Ok(s) => Some((s.to_owned(), false)),
        Err(e) if hit_cap && e.error_len().is_none() => {
            let ok = std::str::from_utf8(&bytes[..e.valid_up_to()]).ok()?;
            Some((ok.to_owned(), false))
        }
        Err(_) if texty_mime => Some((String::from_utf8_lossy(bytes).into_owned(), true)),
        Err(_) => None,
    }
}

fn hex_dump(bytes: &[u8]) -> String {
    crate::hit!("hex_dump");
    use std::fmt::Write;

    let mut out = String::with_capacity(bytes.len() * 4);
    for (i, chunk) in bytes.chunks(16).enumerate() {
        let _ = write!(out, "{:08x}  ", i * 16);
        for j in 0..16 {
            match chunk.get(j) {
                Some(b) => {
                    let _ = write!(out, "{b:02x} ");
                }
                None => out.push_str("   "),
            }
            if j == 7 {
                out.push(' ');
            }
        }
        out.push_str(" |");
        for &b in chunk {
            out.push(if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '.'
            });
        }
        out.push_str("|\n");
    }
    out
}

fn read_head(path: &Path, cap: usize) -> std::io::Result<Vec<u8>> {
    crate::hit!("read_head");
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; cap];
    let mut total = 0;
    loop {
        match f.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                if total >= cap {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    buf.truncate(total);
    Ok(buf)
}

/// Minimum width kept for the file list in wide-preview mode, so names are not trimmed.
const WIDE_PREVIEW_LIST_MIN_WIDTH: i32 = 480;
/// Minimum width always left to the preview panel in wide-preview mode.
const WIDE_PREVIEW_PANEL_MIN_WIDTH: i32 = 360;

/// Width in logical pixels of the monitor the widget is on, or 0 when unknown.
fn monitor_width(widget: &impl IsA<gtk::Widget>) -> i32 {
    use gtk::prelude::*;

    let Some(native) = widget.native() else {
        return 0;
    };
    let Some(surface) = native.surface() else {
        return 0;
    };
    gtk::gdk::Display::default()
        .and_then(|d| d.monitor_at_surface(&surface))
        .map_or(0, |m| m.geometry().width())
}

/// Shows or hides the main column (header bar, tab bar, file grid, status bar) next to the preview.
fn set_main_column_visible(revealer: &gtk::Revealer, visible: bool) {
    crate::hit!("set_main_column_visible");
    use gtk::prelude::*;

    let mut sibling = revealer.parent().and_then(|row| row.first_child());
    while let Some(widget) = sibling {
        if widget.is::<adw::ToolbarView>() {
            widget.set_visible(visible);
        }
        sibling = widget.next_sibling();
    }
}

/// Switches the preview between its capped fixed width and filling the space the list leaves.
fn set_wide_preview_layout(
    revealer: &gtk::Revealer,
    wide: bool,
    list_mode: bool,
    saved_width: i32,
) {
    crate::hit!("set_wide_preview_layout");
    use gtk::prelude::*;

    let row = revealer.parent();
    let total = row.as_ref().map_or(0, |r| r.width());

    // The window can never be wider than the monitor, so half of it caps the fixed panel.
    let max_window = match monitor_width(revealer) {
        0 => total,
        w => w,
    };
    let max_panel = (max_window / 2).max(WIDE_PREVIEW_PANEL_MIN_WIDTH);

    revealer.set_hexpand(wide);
    if let Some(wrapper) = revealer.child() {
        wrapper.set_hexpand(wide);
        wrapper.set_halign(if wide {
            gtk::Align::Fill
        } else {
            gtk::Align::End
        });
        if let Some(clamp) = wrapper.last_child().and_downcast::<adw::Clamp>() {
            // Cap only when wide is off, wide must fill the space the list leaves.
            let cap = if wide { i32::MAX } else { max_panel };
            clamp.set_maximum_size(cap);
            clamp.set_tightening_threshold(cap);
            clamp.set_hexpand(wide);
            if let Some(panel) = clamp.child() {
                panel.set_hexpand(wide);
                let width = if wide {
                    WIDE_PREVIEW_PANEL_MIN_WIDTH
                } else if max_window > 0 {
                    saved_width.clamp(WIDE_PREVIEW_PANEL_MIN_WIDTH, max_panel)
                } else {
                    saved_width
                };
                panel.set_width_request(width);
            }
        }
    }

    let files_min_width = if wide && list_mode && total > 0 {
        (total / 3)
            .max(WIDE_PREVIEW_LIST_MIN_WIDTH)
            .min((total - WIDE_PREVIEW_PANEL_MIN_WIDTH).max(0))
    } else {
        -1
    };

    let mut sibling = row.and_then(|row| row.first_child());
    while let Some(widget) = sibling {
        if widget.is::<adw::ToolbarView>() {
            widget.set_hexpand(!wide);
            widget.set_width_request(files_min_width);
        }
        sibling = widget.next_sibling();
    }
}
