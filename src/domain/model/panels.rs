//! Session state for the ten right-side panels.

use crate::domain::model::RightPanelType;
use gtk::prelude::WidgetExt;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// Per-panel state. Every panel has the three base fields plus [`PanelExtras`].
#[derive(Debug, Default)]
pub struct PanelState {
    /// Revealer wrapping the panel, stored once the window's widgets exist.
    pub revealer: Option<gtk::Revealer>,
    /// Whether the panel is currently open.
    pub visible: bool,
    /// Whether the panel's widget tree has been built (it is built once, then reused).
    pub initialized: bool,
    /// Fields only some panels use.
    pub extras: PanelExtras,
}

/// Per-panel extras. Only the panel that owns a field fills it in.
#[derive(Debug, Default)]
pub struct PanelExtras {
    /// Preview: the panel's `adw::ViewStack`, read from the panel registry, the window
    /// header hosts a `ViewSwitcher` bound to it.
    pub preview_view_stack: Option<adw::ViewStack>,
    /// Preview: the Preview/Details switcher, hoisted into the header bar.
    pub preview_view_switcher: Option<adw::ViewSwitcher>,
    /// Preview: session token for in-flight preview loads. Bumped on every toggle and
    /// every selection change that should invalidate the previous preview.
    pub preview_session: Arc<AtomicU64>,
    /// Diff: text buffer holding the formatted, highlighted git patch.
    pub diff_text_buffer: Option<gtk::TextBuffer>,
    /// Location: path entry, refreshed on each open.
    pub location_entry: Option<gtk::Entry>,
    /// Terminal: the sidebar terminal's `Terminal`, kept alive so its PTY survives toggles.
    pub terminal: Option<crate::infra::terminal::Terminal>,
    /// Terminal: `Some(sidebar_was_visible)` while the panel has the left sidebar hidden.
    pub sidebar_terminal_stashed: Option<bool>,
    /// Quick List: `Some(sidebar_was_visible)` while the panel has the left sidebar hidden.
    pub sidebar_quick_list_stashed: Option<bool>,
    /// Properties: path the panel describes. `None` shows the empty page.
    pub properties_target: Option<PathBuf>,
    /// Properties: session token for in-flight loads, guards stale results.
    pub properties_panel_session: Arc<AtomicU64>,
    /// Actions: session token for in-flight loads, guards stale results.
    pub actions_panel_session: Arc<AtomicU64>,
}

/// One entry per [`RightPanelType`] variant.
#[derive(Debug, Default)]
pub struct Panels {
    pub tag: PanelState,
    pub search: PanelState,
    pub diff: PanelState,
    pub location: PanelState,
    pub connect: PanelState,
    pub preview: PanelState,
    pub actions: PanelState,
    pub quick_list: PanelState,
    pub terminal: PanelState,
    pub properties: PanelState,
}

impl Panels {
    pub fn get(&self, kind: RightPanelType) -> &PanelState {
        match kind {
            RightPanelType::Tag => &self.tag,
            RightPanelType::Search => &self.search,
            RightPanelType::Diff => &self.diff,
            RightPanelType::Location => &self.location,
            RightPanelType::Connect => &self.connect,
            RightPanelType::Preview => &self.preview,
            RightPanelType::Actions => &self.actions,
            RightPanelType::QuickList => &self.quick_list,
            RightPanelType::Terminal => &self.terminal,
            RightPanelType::Properties => &self.properties,
        }
    }

    pub fn get_mut(&mut self, kind: RightPanelType) -> &mut PanelState {
        match kind {
            RightPanelType::Tag => &mut self.tag,
            RightPanelType::Search => &mut self.search,
            RightPanelType::Diff => &mut self.diff,
            RightPanelType::Location => &mut self.location,
            RightPanelType::Connect => &mut self.connect,
            RightPanelType::Preview => &mut self.preview,
            RightPanelType::Actions => &mut self.actions,
            RightPanelType::QuickList => &mut self.quick_list,
            RightPanelType::Terminal => &mut self.terminal,
            RightPanelType::Properties => &mut self.properties,
        }
    }

    /// Every panel with its kind, in the order panels are closed: Tag, Search, Diff,
    /// Location, Connect, Preview, Actions, QuickList, Terminal, Properties.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (RightPanelType, &mut PanelState)> {
        [
            (RightPanelType::Tag, &mut self.tag),
            (RightPanelType::Search, &mut self.search),
            (RightPanelType::Diff, &mut self.diff),
            (RightPanelType::Location, &mut self.location),
            (RightPanelType::Connect, &mut self.connect),
            (RightPanelType::Preview, &mut self.preview),
            (RightPanelType::Actions, &mut self.actions),
            (RightPanelType::QuickList, &mut self.quick_list),
            (RightPanelType::Terminal, &mut self.terminal),
            (RightPanelType::Properties, &mut self.properties),
        ]
        .into_iter()
    }

    /// Whether any panel is open.
    #[allow(dead_code)]
    pub fn any_visible(&self) -> bool {
        [
            &self.tag,
            &self.search,
            &self.diff,
            &self.location,
            &self.connect,
            &self.preview,
            &self.actions,
            &self.quick_list,
            &self.terminal,
            &self.properties,
        ]
        .iter()
        .any(|p| p.visible)
    }

    /// Hides every open panel except `keep`, so at most one panel is open.
    #[must_use = "the caller must run the close hooks for the returned panels"]
    pub fn close_all_except(&mut self, keep: RightPanelType) -> Vec<RightPanelType> {
        let mut closed = Vec::new();
        for (kind, panel) in self.iter_mut() {
            if kind != keep && panel.visible {
                if let Some(ref r) = panel.revealer {
                    r.set_reveal_child(false);
                    r.set_visible(false);
                }
                panel.visible = false;
                closed.push(kind);
            }
        }
        closed
    }
}
