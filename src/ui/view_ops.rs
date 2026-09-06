use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp, SortBy};
use crate::ui::constants;
use crate::utils;
use adw::gdk;
use adw::gio::prelude::*;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use std::sync::atomic::Ordering;

impl FluxApp {
    /// Updates grid selection metadata and formats the status bar label.
    pub fn handle_selection_changed(&mut self, sender: &AsyncComponentSender<Self>) {
        if self.task_queue.summary().is_some() {
            return;
        }

        let mut total_size = 0u64;
        let mut count = 0usize;
        let mut dir_count = 0usize;
        let mut only_files = true;
        let mut only_dirs = true;
        let mut single_name = String::new();

        if let Some(selection_model) = self
            .files
            .view
            .model()
            .and_downcast::<gtk::MultiSelection>()
        {
            let selection = selection_model.selection();
            let n_selected = selection.size();

            for i in 0..n_selected {
                let pos = selection.nth(i as u32);
                if let Some(item_wrapper) = self.files.get(pos) {
                    let item = item_wrapper.borrow();
                    if item.is_dir {
                        only_files = false;
                        dir_count += 1;
                        if count + dir_count == 1 {
                            single_name = item.name.clone();
                        }
                    } else {
                        only_dirs = false;
                        total_size += item.size;
                        count += 1;
                        if count + dir_count == 1 {
                            single_name = item.name.clone();
                        }
                    }
                }
            }
        }

        let total_selected = count + dir_count;

        // Update recents selection flag
        if self.current_path.to_string_lossy() == constants::RECENT_URI {
            self.recents_has_selection = total_selected > 0;
            if self.recents_has_selection {
                self.recents_label = tr("Remove Selected");
                self.recents_tooltip = tr("Remove selected items from recents");
            } else {
                self.recents_label = tr("Clear Recents");
                self.recents_tooltip = tr("Clear all recents");
            }
        }

        self.selection_status = match (total_selected, only_files, only_dirs) {
            (0, _, _) => {
                let child_count = std::fs::read_dir(&self.current_path)
                    .map(|rd| rd.count())
                    .unwrap_or(0);
                let name = self
                    .current_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "/".to_string());
                format!("{} ({} items)", name, child_count)
            }

            // Single file
            (1, true, _) => {
                let size_str = glib::format_size(total_size);
                let selected_item = self
                    .files
                    .view
                    .model()
                    .and_downcast::<gtk::MultiSelection>()
                    .and_then(|m| {
                        let pos = m.selection().nth(0);
                        self.files.get(pos).map(|w| w.borrow().clone())
                    });

                if let Some(ref item) = selected_item {
                    let path = item.path.clone();
                    let s = sender.clone();
                    relm4::spawn_blocking(move || {
                        let mime = utils::get_mime_type(&path);

                        let dimensions = if mime.starts_with("image/") {
                            crate::utils::media::probe_image_dimensions(&path)
                        } else {
                            None
                        };

                        if mime.starts_with("audio/") || mime.starts_with("video/") {
                            let path_c = path.clone();
                            let s_c = s.clone();
                            relm4::spawn(async move {
                                let dur = crate::utils::media::probe_media_duration(&path_c).await;
                                s_c.input(AppMsg::MediaDurationReady(dur));
                            });
                        }

                        s.input(AppMsg::FileMetaReady { mime, dimensions });
                    });
                }

                if let Some(item) = selected_item {
                    let date_str = if item.mtime > 0 {
                        use chrono::TimeZone;
                        match chrono::Local.timestamp_opt(item.mtime, 0) {
                            chrono::LocalResult::Single(dt) => {
                                Some(dt.format("%Y-%m-%d %H:%M").to_string())
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };

                    if let Some(dt_formatted) = date_str {
                        format!("{} ({}) · {}", item.name, size_str, dt_formatted)
                    } else {
                        format!("{} ({})", item.name, size_str)
                    }
                } else {
                    format!("{} ({})", single_name, size_str)
                }
            }

            // Single folder
            (1, _, true) => {
                let item = self
                    .files
                    .view
                    .model()
                    .and_downcast::<gtk::MultiSelection>()
                    .and_then(|m| {
                        let pos = m.selection().nth(0);
                        self.files.get(pos)
                    });
                if let Some(wrapper) = item {
                    let borrowed = wrapper.borrow();
                    let path = borrowed.path.clone();
                    let child_count = std::fs::read_dir(&path).map(|rd| rd.count()).unwrap_or(0);

                    let date_str = if borrowed.mtime > 0 {
                        use chrono::TimeZone;
                        match chrono::Local.timestamp_opt(borrowed.mtime, 0) {
                            chrono::LocalResult::Single(dt) => {
                                Some(dt.format("%Y-%m-%d %H:%M").to_string())
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };

                    if let Some(dt_formatted) = date_str {
                        format!("{} ({} items) · {}", single_name, child_count, dt_formatted)
                    } else {
                        format!("{} ({} items)", single_name, child_count)
                    }
                } else {
                    single_name
                }
            }

            // Multiple files only
            (n, true, _) => {
                let size_str = glib::format_size(total_size);
                format!("{} items ({})", n, size_str)
            }

            // Multiple folders only
            (_, _, true) => format!("{} folders", dir_count),

            // Mixed files + folders
            (_, false, false) => {
                let size_str = glib::format_size(total_size);
                format!("{} folders, {} files ({})", dir_count, count, size_str)
            }
        };
    }

    /// Toggles between grid card layout and compact list view.
    pub fn handle_toggle_list_mode(&mut self) {
        self.is_list_mode = !self.is_list_mode;
        self.saved_list_mode = self.is_list_mode;
        self.config.default_list_mode = self.is_list_mode;
        utils::save_config(&self.config);
        if self.is_list_mode {
            self.files.view.set_min_columns(1);
            self.files.view.set_max_columns(1);
        } else {
            self.files.view.set_min_columns(1);
            self.files.view.set_max_columns(20);
        }
        self.sync_list_mode();
    }

    /// Toggles between ascending and descending sort order.
    pub fn handle_toggle_sort_order(&mut self, sender: &AsyncComponentSender<Self>) {
        self.sort_ascending = !self.sort_ascending;
        let _ = self.state_db.save_view(
            &self.current_path,
            &format!("{:?}", self.sort_by),
            !self.sort_ascending,
            self.current_icon_size as u32,
            self.config.ui.folders_first,
        );
        sender.input(AppMsg::Refresh);
    }

    /// Cycles through available sorting criteria (Name -> Date -> Size -> Type).
    pub fn handle_cycle_sort(&mut self, sender: &AsyncComponentSender<Self>) {
        self.sort_by = match self.sort_by {
            SortBy::Name => SortBy::Date,
            SortBy::Date => SortBy::Size,
            SortBy::Size => SortBy::Type,
            SortBy::Type => SortBy::Name,
        };
        let _ = self.state_db.save_view(
            &self.current_path,
            &format!("{:?}", self.sort_by),
            !self.sort_ascending,
            self.current_icon_size as u32,
            self.config.ui.folders_first,
        );
        sender.input(AppMsg::Refresh);
    }

    /// Toggles "Folders First" sorting priority for the current directory view.
    pub fn handle_cycle_folder_priority(&mut self, sender: &AsyncComponentSender<Self>) {
        let path = self.current_path.clone();
        let current_state = if let Ok(Some((_, _, _, ff))) = self.state_db.get_view(&path) {
            ff
        } else {
            self.config.ui.folders_first
        };
        let new_state = !current_state;

        let _ = self.state_db.save_view(
            &path,
            &format!("{:?}", self.sort_by),
            false,
            self.current_icon_size as u32,
            new_state,
        );
        self.load_path(path, sender);
    }

    /// Adjusts icon scale on scroll, targeting only the active view mode.
    ///
    /// In grid mode the persistent `current_icon_size` and `config.ui.default_icon_size`
    /// are updated. In list mode `current_list_icon_size` and `config.ui.list_icon_size`
    /// are updated instead. Only items matching the current mode have their `icon_size`
    /// field mutated, so switching modes always restores the independent size.
    pub fn handle_zoom(&mut self, delta: f64) {
        if delta == 0.0 {
            return;
        }

        let direction = if delta > 0.0 { -1 } else { 1 };

        if self.is_list_mode {
            let current_size = self.current_list_icon_size;
            let current_idx = constants::CRISP_ICON_SIZES
                .iter()
                .position(|&s| s >= current_size)
                .unwrap_or(constants::CRISP_ICON_SIZES.len() - 1);

            let new_idx = if direction > 0 {
                (current_idx + 1).min(constants::CRISP_ICON_SIZES.len() - 1)
            } else {
                current_idx.saturating_sub(1)
            };

            let new_size = constants::CRISP_ICON_SIZES[new_idx];
            if new_size == self.current_list_icon_size {
                return;
            }

            self.current_list_icon_size = new_size;
            self.config.ui.list_icon_size = new_size;
            utils::save_config(&self.config);

            for i in 0..self.files.len() {
                if let Some(item_wrapper) = self.files.get(i) {
                    if item_wrapper.borrow().is_list_mode {
                        let mut item = item_wrapper.borrow().clone();
                        item.icon_size = new_size;
                        self.files.remove(i);
                        self.files.insert(i, item);
                    }
                }
            }
        } else {
            let current_size = self.current_icon_size;
            let current_idx = constants::CRISP_ICON_SIZES
                .iter()
                .position(|&s| s >= current_size)
                .unwrap_or(constants::CRISP_ICON_SIZES.len() - 1);

            let new_idx = if direction > 0 {
                (current_idx + 1).min(constants::CRISP_ICON_SIZES.len() - 1)
            } else {
                current_idx.saturating_sub(1)
            };

            let new_size = constants::CRISP_ICON_SIZES[new_idx];
            if new_size == self.current_icon_size {
                return;
            }

            self.current_icon_size = new_size;
            let _ = self.state_db.save_view(
                &self.current_path,
                &format!("{:?}", self.sort_by),
                false,
                new_size as u32,
                self.config.ui.folders_first,
            );

            for i in 0..self.files.len() {
                if let Some(item_wrapper) = self.files.get(i) {
                    if !item_wrapper.borrow().is_list_mode {
                        let mut item = item_wrapper.borrow().clone();
                        item.icon_size = new_size;
                        self.files.remove(i);
                        self.files.insert(i, item);
                    }
                }
            }
        }
    }

    /// Receives generated thumbnail textures and updates grid items.
    pub fn handle_thumbnail_ready(&mut self, grid_idx: u32, texture: gdk::Texture, load_id: u64) {
        if load_id != self.load_id.load(Ordering::SeqCst) {
            return;
        }

        if let Some(pos) = (0..self.files.len()).find(|&i| {
            self.files
                .get(i)
                .map(|w| w.borrow().grid_idx == grid_idx)
                .unwrap_or(false)
        }) {
            if let Some(wrapper) = self.files.get(pos) {
                let mut item = wrapper.borrow().clone();
                item.thumbnail = Some(texture.clone());
                let path = item.path.clone();

                self.files.remove(pos);
                self.files.insert(pos, item);

                if let Some(cached) = self.folder_cache.get_mut(&self.current_path) {
                    cached.thumbnails.insert(path, texture);
                }
            }
        }
    }

    pub fn check_visible_thumbnails(&mut self, sender: &AsyncComponentSender<Self>) {
        if !self.config.ui.lazy_thumbnails || self.files.is_empty() {
            return;
        }

        let total_items = self.files.len() as usize;
        let current_session = self.load_id.load(std::sync::atomic::Ordering::SeqCst);

        let vadj = self.files.view.vadjustment();
        let (val, page_size, upper, lower) = match vadj {
            Some(ref adj) => (adj.value(), adj.page_size(), adj.upper(), adj.lower()),
            None => (0.0, 1.0, 1.0, 0.0),
        };

        let max_scroll = (upper - page_size).max(0.0);
        let current_idx = if max_scroll > 0.0 {
            let progress = ((val - lower) / max_scroll).clamp(0.0, 1.0);
            (progress * (total_items.saturating_sub(1) as f64)).round() as usize
        } else {
            0
        };

        let last_idx = self.last_thumb_scroll_idx;
        self.last_thumb_scroll_idx = current_idx;

        let window_size = 60usize.min(total_items);
        let min_pos = current_idx.min(last_idx).saturating_sub(window_size / 2);
        let max_pos = (current_idx.max(last_idx) + window_size).min(total_items);

        for i in min_pos..max_pos {
            if let Some(wrapper) = self.files.get(i as u32) {
                let item = wrapper.borrow();
                let grid_idx = item.grid_idx;

                if self.pending_thumbnails.contains(&grid_idx) {
                    continue;
                }

                if !item.is_dir && item.thumbnail.is_none() {
                    let (is_img, is_vid) = utils::is_visual_media(&item.path);
                    if is_img || is_vid {
                        self.pending_thumbnails.insert(grid_idx);
                        self.spawn_single_thumbnail(
                            grid_idx,
                            item.path.clone(),
                            current_session,
                            sender.clone(),
                        );
                    }
                }
            }
        }
    }

    /// Appends audio/video duration strings to the status bar.
    pub fn handle_media_duration_ready(&mut self, maybe_duration: Option<std::time::Duration>) {
        if self.task_queue.summary().is_none()
            && !self.selection_status.starts_with('[')
            && !self.selection_status.is_empty()
        {
            if let Some(dur) = maybe_duration {
                let dur_str = crate::utils::media::format_duration(dur);
                if !self.selection_status.contains(&dur_str) {
                    self.selection_status.push_str(&format!(" - {}", dur_str));
                }
            }
        }
    }

    /// Appends image dimensions and MIME type labels to the status bar.
    pub fn handle_file_meta_ready(&mut self, mime: String, dimensions: Option<(u32, u32)>) {
        if self.task_queue.summary().is_some() || self.selection_status.is_empty() {
            return;
        }

        if self.selection_status.starts_with('[')
            || self.selection_status.contains("items")
            || self.selection_status.contains("folders")
        {
            return;
        }

        let dim_str = dimensions.map(|(w, h)| {
            let ratio = crate::utils::media::aspect_ratio_label(w, h);
            format!(" - {}×{} ({})", w, h, ratio)
        });
        if let Some(d) = dim_str {
            self.selection_status.push_str(&d);
        }

        self.selection_status.push_str(&format!(" - {}", mime));
    }

    /// Handles an on-demand thumbnail request dispatched by `FileItem::bind`.
    ///
    /// Only active when `config.ui.lazy_thumbnails` is `true`.  Guards against
    /// duplicate jobs using `pending_thumbnails`: the first `bind()` call for a
    /// given `grid_idx` in the current session inserts it into the set and
    /// spawns exactly one background worker, subsequent calls for the same index
    /// (widget recycling, re-bind during scroll) are no-ops.
    pub fn handle_request_thumbnail(
        &mut self,
        grid_idx: u32,
        path: std::path::PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let current_session = self.load_id.load(std::sync::atomic::Ordering::SeqCst);

        if !self.pending_thumbnails.insert(grid_idx) {
            return;
        }

        self.spawn_single_thumbnail(grid_idx, path, current_session, sender.clone());
    }
}
