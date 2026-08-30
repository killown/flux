use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use crate::utils;
use adw::gdk;
use adw::gio::prelude::*;
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Dispatches opening for a specific grid position or current multi-selection.
    pub fn handle_open(&self, position: Option<u32>, sender: &AsyncComponentSender<Self>) {
        let modifiers = gdk::Display::default()
            .and_then(|d| d.default_seat())
            .and_then(|s| s.keyboard())
            .map(|k| k.modifier_state())
            .unwrap_or(gdk::ModifierType::empty());

        let is_selecting =
            modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK);
        if is_selecting {
            return;
        }

        let items: Vec<(PathBuf, bool)> = if let Some(pos) = position {
            let query_lc = self.filter.to_lowercase();

            if self.filter.is_empty() || self.is_content_searching {
                self.files
                    .get(pos)
                    .map(|w| {
                        let item = w.borrow();
                        vec![(item.path.clone(), item.is_dir)]
                    })
                    .unwrap_or_default()
            } else if let Some((tags, rest)) = crate::utils::search::parse_tag_filter(&query_lc) {
                // If tag search rebuilt the files list directly, map by position first
                if let Some(wrapper) = self.files.get(pos) {
                    let item = wrapper.borrow();
                    vec![(item.path.clone(), item.is_dir)]
                } else {
                    let rest_clean = rest.trim().to_lowercase();
                    let target_tags: Vec<String> =
                        tags.into_iter().map(|t| t.to_lowercase()).collect();
                    let mut match_count = 0u32;
                    let mut found = None;

                    for i in 0..self.files.len() {
                        if let Some(wrapper) = self.files.get(i) {
                            let item = wrapper.borrow();
                            let name_ok = rest_clean.is_empty()
                                || item.name.to_lowercase().contains(&rest_clean);

                            if name_ok {
                                let file_tags = crate::utils::xattr::read_tags(&item.path);
                                let file_tags_lc: Vec<String> =
                                    file_tags.into_iter().map(|t| t.to_lowercase()).collect();

                                if target_tags.iter().all(|req| file_tags_lc.contains(req)) {
                                    if match_count == pos {
                                        found = Some((item.path.clone(), item.is_dir));
                                        break;
                                    }
                                    match_count += 1;
                                }
                            }
                        }
                    }
                    found.map(|f| vec![f]).unwrap_or_default()
                }
            } else if let Some((op, rest)) = crate::utils::search::parse_size_filter(&query_lc) {
                let rest_clean = rest.trim().to_lowercase();
                let mut match_count = 0u32;
                let mut found = None;

                for i in 0..self.files.len() {
                    if let Some(wrapper) = self.files.get(i) {
                        let item = wrapper.borrow();
                        let name_ok =
                            rest_clean.is_empty() || item.name.to_lowercase().contains(&rest_clean);
                        let size_ok = if item.is_dir {
                            true
                        } else {
                            match op {
                                crate::utils::search::SizeOp::Gt(v) => item.size > v,
                                crate::utils::search::SizeOp::Lt(v) => item.size < v,
                                crate::utils::search::SizeOp::Range(l, r) => {
                                    item.size >= l && item.size <= r
                                }
                            }
                        };

                        if name_ok && size_ok {
                            if match_count == pos {
                                found = Some((item.path.clone(), item.is_dir));
                                break;
                            }
                            match_count += 1;
                        }
                    }
                }
                found.map(|f| vec![f]).unwrap_or_default()
            } else {
                let mut match_count = 0u32;
                let mut found = None;
                for i in 0..self.files.len() {
                    if let Some(wrapper) = self.files.get(i) {
                        let item = wrapper.borrow();
                        if item.name.to_lowercase().contains(&query_lc) {
                            if match_count == pos {
                                found = Some((item.path.clone(), item.is_dir));
                                break;
                            }
                            match_count += 1;
                        }
                    }
                }
                found.map(|f| vec![f]).unwrap_or_default()
            }
        } else {
            self.get_selection_with_meta()
        };

        if items.is_empty() {
            return;
        }

        self.activate_items(items, sender);
    }

    /// Handles explicit primary item activation (e.g. Return / Double-click).
    pub fn handle_activate(&self, sender: &AsyncComponentSender<Self>) {
        let modifiers = gdk::Display::default()
            .and_then(|d| d.default_seat())
            .and_then(|s| s.keyboard())
            .map(|k| k.modifier_state())
            .unwrap_or(gdk::ModifierType::empty());

        let is_selecting =
            modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK);
        if is_selecting {
            return;
        }

        let items = self.get_selection_with_meta();
        if items.is_empty() {
            return;
        }

        self.activate_items(items, sender);
    }

    /// Launches selection using a Desktop Entry ID.
    pub fn handle_launch_with_app(&self, app_id: String) {
        if let Some(app_info) = gio_unix::DesktopAppInfo::new(&app_id) {
            let selection = self.get_selection();
            if selection.is_empty() {
                return;
            }

            let files: Vec<gio::File> = selection.into_iter().map(gio::File::for_path).collect();
            let context = gdk::Display::default().map(|display| display.app_launch_context());
            let launch_result = app_info.launch(&files, context.as_ref());

            if let Err(e) = launch_result {
                eprintln!("[Launch Error] {:?}: {}", app_info.display_name(), e);
            }
        }
    }

    /// Clears selected or all entries from GTK recents.
    pub fn handle_clear_recents(&mut self, sender: &AsyncComponentSender<Self>) {
        let selection = self.get_selection();
        let result = if selection.is_empty() {
            crate::utils::remove_recents(None)
        } else {
            crate::utils::remove_recents(Some(&selection))
        };

        match result {
            Ok(()) => {
                sender.input(AppMsg::Refresh);
                self.recents_has_selection = false;
            }
            Err(e) => {
                sender.input(AppMsg::ShowToast(format!("Failed to clear recents: {}", e)));
            }
        }
    }

    /// Resolves target selection coordinates and triggers background MIME calculation for the context menu.
    pub fn handle_prepare_context_menu(
        &self,
        x: f64,
        y: f64,
        item_idx: Option<u32>,
        sender: &AsyncComponentSender<Self>,
    ) {
        let path = if let Some(idx) = item_idx {
            if let Some(selection_model) = self
                .files
                .view
                .model()
                .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
            {
                if !selection_model.selection().contains(idx) {
                    selection_model.select_item(idx, true);
                }
            }
            self.files.get(idx).map(|w| w.borrow().path.clone())
        } else {
            None
        };

        let sender_ctx = sender.clone();
        relm4::spawn_blocking(move || {
            let mime = path
                .as_ref()
                .map(|p| utils::get_mime_type(p))
                .unwrap_or_else(|| constants::MIME_DIR.to_string());

            sender_ctx.input(AppMsg::ShowContextMenu { x, y, path, mime });
        });
    }
}
