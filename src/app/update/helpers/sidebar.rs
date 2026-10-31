//! Sidebar rebuilding and the right-side panel toggle.

use crate::app::state::FluxApp;
use crate::domain::model::constants;
use crate::domain::model::{
    AppMsg, DeviceMsg, NavigationMsg, QuickListMsg, RightPanelType, SearchMsg,
};
use crate::ui::SidebarPlace;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

fn terminal_default_width(anchor: &gtk::Revealer) -> i32 {
    let monitor_w = anchor
        .native()
        .and_then(|n| n.surface())
        .and_then(|s| gtk::gdk::Display::default().and_then(|d| d.monitor_at_surface(&s)))
        .map(|m| m.geometry().width())
        .unwrap_or(1920);

    let scale = (monitor_w as f64 / 1920.0).clamp(1.0, 2.0);
    (800.0 * scale) as i32
}

impl FluxApp {
    /// Rebuilds the sidebar navigation list from XDG directories, mounts, and configuration.
    pub fn refresh_sidebar(&mut self) {
        crate::hit!("refresh_sidebar");
        let mut guard = self.sidebar.guard();
        guard.clear();

        let recents_place = || crate::ui::SidebarPlace {
            name: crate::i18n::tr("Recents"),
            icon: "document-open-recent-symbolic".to_string(),
            path: std::path::PathBuf::from(constants::RECENT_URI),
            is_mount: false,
            is_section_label: false,
        };

        let get_xdg_name = |p: &PathBuf| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.to_string_lossy().to_string())
        };

        if self.config.ui.show_xdg_dirs {
            if let Some(p) = dirs::home_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "user-home-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::desktop_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "user-desktop-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::download_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-download-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::document_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-documents-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::picture_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-pictures-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::video_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-videos-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
        }

        let show_recents = self.config.ui.show_recents;
        let recents_row = self.config.ui.recents_row;
        for (idx, custom) in self.config.sidebar.iter().enumerate() {
            if show_recents && idx == recents_row {
                guard.push_back(recents_place());
            }
            if custom.kind.as_deref() == Some("label") {
                guard.push_back(SidebarPlace {
                    name: custom.name.clone(),
                    icon: String::new(),
                    path: PathBuf::new(),
                    is_mount: false,
                    is_section_label: true,
                });
                continue;
            }

            let path = if custom.path.starts_with('~') {
                dirs::home_dir()
                    .map(|h| PathBuf::from(custom.path.replace('~', &h.to_string_lossy())))
                    .unwrap_or_else(|| PathBuf::from(&custom.path))
            } else {
                PathBuf::from(&custom.path)
            };

            let mut name = custom.name.clone();
            if custom.path == crate::infra::trash::TRASH_URI && name == "Trash" {
                name = crate::i18n::tr("Trash");
            } else if custom.path == "tags://" && name == "Tags" {
                name = crate::i18n::tr("Tags");
            }

            guard.push_back(SidebarPlace {
                name,
                icon: custom.icon.clone(),
                path,
                is_mount: false,
                is_section_label: false,
            });
        }

        if show_recents && recents_row >= self.config.sidebar.len() {
            guard.push_back(recents_place());
        }

        // Mounts - Offloaded to background thread to prevent blocking the UI loop
        if let Some(sender) = crate::infra::SENDER.get().cloned() {
            relm4::spawn_blocking(move || {
                let mounts = crate::infra::mounts::get_system_mounts();
                let _ = sender.send(AppMsg::Device(DeviceMsg::SystemMountsReady(mounts)));
            });
        }
    }

    /// Hides the left sidebar while the Quick List panel is open, remembering
    /// whether it was showing.
    fn stash_sidebar_for_quick_list(&mut self) {
        if self
            .panels
            .quick_list
            .extras
            .sidebar_quick_list_stashed
            .is_none()
        {
            self.panels.quick_list.extras.sidebar_quick_list_stashed = Some(self.sidebar_visible);
        }
        if self.sidebar_visible {
            self.sidebar_visible = false;
            if let Some(ref w) = self.sidebar_widget {
                w.set_visible(false);
            }
        }
    }

    /// Brings the left sidebar back after the Quick List panel closed, but only
    /// if it was showing before and the config still says to show it.
    fn restore_sidebar_after_quick_list(&mut self) {
        if let Some(was_visible) = self
            .panels
            .quick_list
            .extras
            .sidebar_quick_list_stashed
            .take()
        {
            if was_visible && self.config.ui.sidebar_visible {
                self.sidebar_visible = true;
                if let Some(ref w) = self.sidebar_widget {
                    w.set_visible(true);
                }
            }
        }
    }

    /// Opens `kind`, or closes it if it is already open.
    ///
    /// Only one right-side panel is open at a time: every other open panel is closed first
    /// (`Panels::close_all_except`) and gets its close hook.
    pub fn toggle_sidebar_right_panel(
        &mut self,
        kind: RightPanelType,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("toggle_sidebar_right_panel");
        // Ensure mutually exclusive right-side panels
        for other in self.panels.close_all_except(kind) {
            self.on_panel_close(other, CloseReason::Displaced, sender);
        }

        let Some(revealer) = self.panels.get(kind).revealer.clone() else {
            return;
        };

        if !self.panels.get(kind).initialized {
            let built = self.build_panel(kind, &revealer, sender);
            revealer.set_child(Some(&built));
            self.panels.get_mut(kind).initialized = true;
        } else if kind == RightPanelType::Location && !self.panels.location.visible {
            // Panel is built once, so refresh the entry with the current path on each open.
            if let Some(ref entry) = self.panels.location.extras.location_entry {
                entry.set_text(&self.current_path.to_string_lossy());
            }
        }

        let panel = self.panels.get_mut(kind);
        let now_visible = !panel.visible;
        panel.visible = now_visible;
        revealer.set_visible(now_visible);
        revealer.set_reveal_child(now_visible);

        if now_visible {
            self.on_panel_open(kind, &revealer, sender);
        } else {
            self.on_panel_close(kind, CloseReason::Toggled, sender);
            // FOCUS: when a right panel closes, its input widget is hidden/destroyed, causing
            // GTK's focus chain to jump to the first focusable widget (the sidebar), which
            // paints an ugly focus outline square. Pull focus back to the main file grid.
            if let Some(tab) = self.tabs.get(self.active_tab_index) {
                tab.files.view.grab_focus();
            }
        }
    }

    /// Builds the widget tree for `kind`, the first time it is opened.
    ///
    /// The caller attaches the result to the panel's revealer and marks it initialized.
    /// Panels that keep handles into their tree (diff buffer, location entry, terminal)
    /// store them in `self.panels` here.
    fn build_panel(
        &mut self,
        kind: RightPanelType,
        revealer: &gtk::Revealer,
        sender: &AsyncComponentSender<Self>,
    ) -> gtk::Box {
        crate::hit!("build_panel");
        match kind {
            RightPanelType::Connect => crate::ui::panels::build_connect_panel(sender.clone()),
            RightPanelType::Tag => {
                let tags = self.state_db.list_all_tags().unwrap_or_default();
                crate::ui::panels::build_tag_panel(
                    tags,
                    self.config.ui.tag_panel_width,
                    sender.clone(),
                )
            }
            RightPanelType::Search => crate::ui::panels::build_search_panel(
                self.config.ui.search_panel_width,
                sender.clone(),
            ),
            RightPanelType::Diff => {
                let buf = gtk::TextBuffer::new(None);
                self.panels.diff.extras.diff_text_buffer = Some(buf.clone());
                crate::ui::panels::build_diff_panel(
                    self.config.ui.diff_panel_width,
                    buf,
                    sender.clone(),
                )
            }
            RightPanelType::Location => {
                let (panel, entry) = crate::ui::panels::build_location_panel(
                    &self.current_path.to_string_lossy(),
                    self.state_db.clone(),
                    sender.clone(),
                );
                self.panels.location.extras.location_entry = Some(entry);
                panel
            }
            // `build_preview_panel` installs its own handle into the thread-local registry so
            // `handle_preview_ready` can push content without threading a handle through
            // `FluxApp`. The `view_stack` and `switcher` are pulled out of the registry so the
            // header bar's reactive bindings can pick them up.
            RightPanelType::Preview => {
                let panel = crate::ui::panels::build_preview_panel(
                    self.config.ui.preview_panel_width,
                    sender.clone(),
                );
                self.refresh_preview_header_widgets();
                panel
            }
            // `build_actions_panel` installs its list/search/stack handles into the
            // thread-local `actions_registry`.
            RightPanelType::Actions => crate::ui::panels::build_actions_panel(
                self.config.ui.actions_panel_width,
                sender.clone(),
            ),
            // The rows are drawn by `RebuildQuickPanel`, queued here so it runs once the panel
            // is attached.
            RightPanelType::QuickList => {
                sender.input(AppMsg::QuickList(QuickListMsg::RebuildQuickPanel));
                crate::ui::panels::build_quick_list_panel(0, sender.clone())
            }
            // Build the widget tree and keep the `Terminal`. The shell is *not* spawned here:
            // `on_panel_open` spawns it every time the panel is opened, so that closing and
            // reopening always yields a live PTY instead of a dead one.
            RightPanelType::Terminal => {
                let default_width = terminal_default_width(revealer);
                let (root, term) = crate::ui::panels::build_terminal_panel(
                    &self.config.ui.terminal,
                    default_width,
                    sender.clone(),
                );
                // The terminal is created lazily, long after `setup_window`, so its shutdown
                // hooks are registered here, once it exists.
                crate::ui::startup::register_terminal_shutdown(&term, revealer);
                self.panels.terminal.extras.terminal = Some(term);
                root
            }
            // `build_properties_panel` installs its handles into a thread-local, the first load
            // is requested by `on_panel_open`.
            RightPanelType::Properties => {
                crate::ui::panels::build_properties_panel(0, sender.clone())
            }
        }
    }

    /// Runs after `kind` became visible: sidebar stashes, shell spawn, loads, focus.
    fn on_panel_open(
        &mut self,
        kind: RightPanelType,
        revealer: &gtk::Revealer,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("on_panel_open");
        match kind {
            // Tag, Search, Diff, Location and Connect focus their first entry.
            RightPanelType::Tag
            | RightPanelType::Search
            | RightPanelType::Diff
            | RightPanelType::Location
            | RightPanelType::Connect => focus_first_entry(revealer),
            // The preview has no entry and must not steal focus from the grid: grabbing focus
            // here would break arrow-key scrubbing while the panel is open. While it is
            // visible the header stack shows the `"preview-switcher"` page instead of the
            // breadcrumbs.
            RightPanelType::Preview => self.refresh_preview_header_widgets(),
            // The contextual-actions panel also follows grid selection, so it must not steal
            // focus either. Opening loads the actions for the current selection.
            RightPanelType::Actions => self.request_actions_panel_load(sender),
            // The Quick List panel takes the left sidebar's place while it is open.
            RightPanelType::QuickList => self.stash_sidebar_for_quick_list(),
            // Spawn the shell on every reveal, kill it on every hide. This is what makes `F4`
            // (toggle) behave correctly: the shell is not reused across opens, so its PTY fd
            // is never stale. The terminal skips focus: the user usually wants to keep
            // navigating the grid while the shell runs on the side.
            RightPanelType::Terminal => self.open_terminal_shell(),
            // Loads the metadata for the selection (unless `OpenFileProperties` already chose
            // a target).
            RightPanelType::Properties => {
                if self.panels.properties.extras.properties_target.is_none() {
                    self.panels.properties.extras.properties_target = self.get_selected_path();
                }
                self.request_properties_panel_load(sender);
            }
        }
    }

    /// Runs after `kind` became hidden, either toggled shut or displaced by another panel.
    ///
    /// Restores sidebars, kills the shell, stops media and drops in-flight loads. The two
    /// reasons differ for Search, Tag (content search is only cancelled on a toggle) and
    /// Preview (wide mode is only left on a displacement, as before).
    fn on_panel_close(
        &mut self,
        kind: RightPanelType,
        reason: CloseReason,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("on_panel_close");
        match kind {
            RightPanelType::Search if reason == CloseReason::Toggled => {
                if self.search.content_searching || self.search.last_was_advanced {
                    self.search.last_was_advanced = false;
                    sender.input(AppMsg::Search(SearchMsg::CancelContentSearch));
                    sender.input(AppMsg::Search(SearchMsg::ClearExtensionFilter));
                    sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
                }
            }
            RightPanelType::Tag if reason == CloseReason::Toggled => {
                sender.input(AppMsg::Search(SearchMsg::CancelContentSearch));
                sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
            }
            RightPanelType::Diff => self.git.active_diff_target = None,
            // Previews own live media (video/audio), stop it on close so nothing keeps playing
            // after the panel is hidden.
            RightPanelType::Preview => {
                crate::ui::panels::preview::registry::with(|h| h.stop_media());
                self.stop_video_preview();
                match reason {
                    CloseReason::Displaced => self.exit_wide_preview(),
                    CloseReason::Toggled => self.refresh_preview_header_widgets(),
                }
            }
            // Closing bumps the session so any in-flight result is dropped and empties the list.
            RightPanelType::Actions => self.reset_actions_panel(),
            RightPanelType::QuickList => self.restore_sidebar_after_quick_list(),
            RightPanelType::Terminal => self.close_terminal_shell(),
            // Closing bumps the session so late results are dropped.
            RightPanelType::Properties => self.reset_properties_panel(),
            // Displaced Search/Tag panels leave any content search running, as before.
            RightPanelType::Search
            | RightPanelType::Tag
            | RightPanelType::Location
            | RightPanelType::Connect => {}
        }
    }

    /// Re-reads the preview `ViewStack` and `ViewSwitcher` from the panel registry, for the
    /// header bar's reactive bindings.
    fn refresh_preview_header_widgets(&mut self) {
        let extras = &mut self.panels.preview.extras;
        extras.preview_view_stack = crate::ui::panels::preview::registry::get_view_stack();
        extras.preview_view_switcher = crate::ui::panels::preview::registry::get_view_switcher();
    }

    /// Stashes and hides the left sidebar so the terminal has room, then respawns the shell
    /// in the current directory.
    fn open_terminal_shell(&mut self) {
        if self
            .panels
            .terminal
            .extras
            .sidebar_terminal_stashed
            .is_none()
        {
            self.panels.terminal.extras.sidebar_terminal_stashed = Some(self.sidebar_visible);
        }
        if self.sidebar_visible {
            self.sidebar_visible = false;
            if let Some(ref w) = self.sidebar_widget {
                w.set_visible(false);
            }
        }

        if let Some(ref term) = self.panels.terminal.extras.terminal {
            term.kill_shell();
            let path_str = self.current_path.to_string_lossy();
            let working_dir = if path_str.starts_with(crate::infra::archive::ARCHIVE_URI) {
                crate::infra::archive::parse_archive_uri(&path_str)
                    .map(|(archive_path, _)| {
                        archive_path
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "/".to_string())
                    })
                    .unwrap_or_else(|| "/".to_string())
            } else if crate::infra::network::is_network_uri(&self.current_path) {
                std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
            } else {
                path_str.to_string()
            };
            let mut term_spawn = term.clone();
            term_spawn.spawn_async(
                0,
                Some(&working_dir),
                &[],
                &[],
                0,
                || {},
                -1,
                None,
                |result| {
                    if let Err(e) = result {
                        eprintln!("[flux] sidebar terminal spawn: {e}");
                    }
                },
            );
            let da = term.drawing_area.clone();
            gtk::glib::idle_add_local_once(move || {
                da.grab_focus();
            });
        }
    }

    /// Restores the left sidebar the user had when the terminal opened and kills the shell.
    fn close_terminal_shell(&mut self) {
        if let Some(was_visible) = self.panels.terminal.extras.sidebar_terminal_stashed.take() {
            self.sidebar_visible = was_visible;
            if let Some(ref w) = self.sidebar_widget {
                w.set_visible(was_visible);
            }
        }
        if let Some(ref term) = self.panels.terminal.extras.terminal {
            term.kill_shell();
        }
    }
}

/// Why a right-side panel closed.
///
/// The old code ran slightly different hooks for the two paths, and `on_panel_close` keeps
/// that difference.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CloseReason {
    /// The user toggled the panel shut.
    Toggled,
    /// Another panel opened and displaced it.
    Displaced,
}

/// Focuses the first `gtk::Entry` or `gtk::SearchEntry` in the panel (looking one level
/// down), or the panel box itself when there is none.
fn focus_first_entry(revealer: &gtk::Revealer) {
    let Some(panel_box) = revealer.child() else {
        return;
    };
    let mut next = panel_box.first_child();
    let mut focused = false;
    while let Some(w) = next {
        if let Some(entry) = w.downcast_ref::<gtk::Entry>() {
            entry.grab_focus();
            focused = true;
            break;
        }
        if let Some(entry) = w.downcast_ref::<gtk::SearchEntry>() {
            entry.grab_focus();
            focused = true;
            break;
        }
        if let Some(inner) = w.first_child() {
            if let Some(entry) = inner.downcast_ref::<gtk::Entry>() {
                entry.grab_focus();
                focused = true;
                break;
            }
            if let Some(entry) = inner.downcast_ref::<gtk::SearchEntry>() {
                entry.grab_focus();
                focused = true;
                break;
            }
        }
        next = w.next_sibling();
    }
    if !focused {
        panel_box.grab_focus();
    }
}
