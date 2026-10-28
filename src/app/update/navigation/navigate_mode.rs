//! Navigate mode: a session stance where a plain letter enters the folders
//! starting with it, cycling on repeats. It is the `Ctrl+Alt+<letter>` cycle
//! (`handle_cycle_by_letter`) without the chord.
//!
//! The state is [`crate::model::NavigateMode`] on `FluxApp`. The mode is
//! entered and left by a chord (or Esc) and never auto-entered by focus.
//! Text entries and the terminal are filtered out before a key becomes
//! `AppMsg::KeyChar` / `KeyBackspace` / `KeyEscape`, so typing into the search
//! panel, rename entry, location panel or terminal is unaffected.

use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp, NavigateMode};
use crate::ui::constants;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::path::{Path, PathBuf};

impl FluxApp {
    /// Turns Navigate mode on, or off when it is already on.
    pub fn handle_toggle_navigate_mode(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_toggle_navigate_mode");
        if self.navigate.is_some() {
            self.exit_navigate_mode(sender);
        } else {
            self.enter_navigate_mode();
        }
    }

    /// Enters the mode. A live search-bar filter is stashed and cleared so the
    /// whole folder stays reachable, leaving the mode puts it back.
    fn enter_navigate_mode(&mut self) {
        crate::hit!("enter_navigate_mode");
        let stashed_filter = if !self.filter.is_empty() && !self.is_content_searching {
            Some(self.filter.clone())
        } else {
            None
        };
        if self.header_view == constants::VIEW_SEARCH || !self.filter.is_empty() {
            self.handle_switch_header(constants::VIEW_PATH.to_string());
        }
        self.alt_cycle = None;
        self.navigate = Some(NavigateMode {
            anchors: Vec::new(),
            stashed_filter,
            select_first: true,
            auto_selected: None,
        });
        // A folder still loading selects once it lands, see `handle_folder_loaded`.
        if !self.is_loading {
            self.select_first_for_navigate();
        }
    }

    /// Owes the next finished folder load a first-item selection. Called when a
    /// navigation starts, so entering a folder in Navigate mode lands on its
    /// first item. No-op outside the mode.
    pub(super) fn navigate_owe_first_selection(&mut self) {
        if let Some(state) = self.navigate.as_mut() {
            state.select_first = true;
        }
    }

    /// Selects and focuses the first grid item when Navigate mode is owed one,
    /// so the arrow keys work right away.
    ///
    /// One-shot: the debt is cleared even when the folder is empty, so a later
    /// refresh never moves the selection.
    pub fn select_first_for_navigate(&mut self) {
        crate::hit!("select_first_for_navigate");
        match self.navigate.as_mut() {
            Some(state) if state.select_first => state.select_first = false,
            _ => return,
        }
        let view = self.tabs[self.active_tab_index].files.view.clone();
        let Some(model) = view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
        else {
            return;
        };
        if model.n_items() == 0 {
            return;
        }
        model.unselect_all();
        model.select_item(0, true);
        let selected = self.get_selected_path();
        if let Some(state) = self.navigate.as_mut() {
            state.auto_selected = selected;
        }
        // Deferred to idle so the grid allocation settles before it takes focus.
        glib::idle_add_local_once(move || {
            view.scroll_to(
                0,
                gtk::ListScrollFlags::FOCUS | gtk::ListScrollFlags::SELECT,
                None,
            );
        });
    }

    /// True once the user moved the selection off the item Navigate mode picked
    /// by itself, so Enter opens that item instead of rebasing the cycle.
    fn navigate_selection_moved(&self) -> bool {
        let selected = self.get_selected_path();
        let auto = self
            .navigate
            .as_ref()
            .and_then(|s| s.auto_selected.as_ref());
        selected.is_some() && selected.as_ref() != auto
    }

    /// Leaves the mode and restores the stashed filter, unless a newer filter
    /// was started meanwhile.
    fn exit_navigate_mode(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("exit_navigate_mode");
        let Some(state) = self.navigate.take() else {
            return;
        };
        self.alt_cycle = None;
        if let Some(filter) = state.stashed_filter {
            if self.filter.is_empty() && !filter.is_empty() {
                self.header_view = constants::VIEW_SEARCH.to_string();
                self.filter = filter.clone();
                sender.input(AppMsg::UpdateFilter(filter));
            }
        }
    }

    /// A plain printable key reached the grid: enter/cycle folders in Navigate
    /// mode, type-to-search otherwise.
    pub fn handle_key_char(&mut self, c: char, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_key_char");
        if self.navigate.is_some() {
            if c.is_ascii_alphanumeric() {
                self.handle_cycle_by_letter(c, sender);
            }
            return;
        }
        sender.input(AppMsg::SwitchHeader(constants::VIEW_SEARCH.to_string()));
        sender.input(AppMsg::SearchInput(c));
    }

    /// Enter on the grid. In Navigate mode, while inside a folder reached by
    /// cycling, the folder becomes the new base: the next letters cycle its
    /// subfolders, and the old base is remembered for Backspace. Once an arrow
    /// key has moved the selection, Enter opens the selected item instead.
    pub fn handle_key_enter(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_key_enter");
        let inside = self.alt_cycle.as_ref().is_some_and(|c| {
            c.candidates
                .iter()
                .any(|p| Self::cycle_inside(&self.current_path, p))
        });
        if self.navigate.is_some() && inside && !self.navigate_selection_moved() {
            if let (Some(cycle), Some(state)) = (self.alt_cycle.take(), self.navigate.as_mut()) {
                state.anchors.push(cycle.anchor);
            }
            return;
        }
        sender.input(AppMsg::Activate);
    }

    /// Backspace on the grid. In Navigate mode it first abandons a running
    /// cycle (back to the folder it started from), with no cycle running it
    /// steps back to the previous base, or the parent when there is none.
    pub fn handle_key_backspace(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_key_backspace");
        if self.navigate.is_some() {
            if self.alt_cycle.is_some() {
                self.handle_cycle_by_letter_cancel(sender);
                return;
            }
            let previous = self
                .navigate
                .as_mut()
                .and_then(|state| state.anchors.pop())
                .or_else(|| Self::navigate_parent(&self.current_path));
            if let Some(path) = previous {
                sender.input(AppMsg::Navigate(path));
            }
            return;
        }
        sender.input(AppMsg::SearchBackspace);
    }

    /// Esc on the grid.
    pub fn handle_key_escape(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_key_escape");
        // Esc closes the Quick List panel. The lists are kept.
        if self.quick_list_panel_visible {
            sender.input(AppMsg::ToggleQuickListPanel);
        }
        if self.navigate.is_some() {
            self.exit_navigate_mode(sender);
            return;
        }
        sender.input(AppMsg::UpdateFilter(String::new()));
        sender.input(AppMsg::ClearMimeFilter);
        sender.input(AppMsg::SwitchHeader(constants::VIEW_PATH.to_string()));
    }

    /// The folder one level up from `current`.
    ///
    /// Inside an `archive://` location a plain `Path::parent` would cut the
    /// URI apart. There the parent is the enclosing folder inside the archive,
    /// and from the archive root it is the folder holding the archive file.
    fn navigate_parent(current: &Path) -> Option<PathBuf> {
        crate::hit!("navigate_parent");
        let Some((archive, inner)) =
            crate::infra::archive::parse_archive_uri(&current.to_string_lossy())
        else {
            return current.parent().map(PathBuf::from);
        };
        let inner = inner.trim_matches('/');
        if inner.is_empty() {
            return archive.parent().map(PathBuf::from);
        }
        let up = inner.rsplit_once('/').map_or("", |(head, _)| head);
        Some(crate::infra::archive::build_archive_uri(&archive, up))
    }

    /// Status-bar chip text: `Navigate` or `Navigate: d` for the letter being cycled.
    pub fn navigate_chip_label(&self) -> String {
        match &self.alt_cycle {
            Some(cycle) => format!("{}: {}", tr("Navigate"), cycle.letter),
            None => tr("Navigate"),
        }
    }
}
