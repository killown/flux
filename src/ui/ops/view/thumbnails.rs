use crate::model::FluxApp;
use crate::utils;
use adw::gdk;
use adw::prelude::*;
use relm4::prelude::*;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn handle_update_visible_thumbnails_viewport(
        &mut self,
        progress_top: f64,
        progress_bottom: f64,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_update_visible_thumbnails_viewport");
        let active_files = &self.tabs[self.active_tab_index].files;

        if !self.config.ui.lazy_thumbnails || active_files.is_empty() {
            return;
        }

        let total_items = active_files.len() as usize;
        let max_item_idx = total_items.saturating_sub(1);

        let first_visible = (progress_top * max_item_idx as f64).floor() as usize;
        let last_visible = (progress_bottom * max_item_idx as f64).ceil() as usize;

        let overscan = 30usize;
        let visible_start = first_visible.saturating_sub(overscan) as u32;
        let visible_end = (last_visible + overscan).min(max_item_idx) as u32;

        let cancelled = self
            .thumbnail_manager
            .cancel_out_of_viewport(visible_start, visible_end);
        for idx in cancelled {
            self.pending_thumbnails.remove(&idx);
        }

        let max_threads = self.config.ui.thumbnail_threads.max(1);
        let sem = self.thumbnail_manager.get_semaphore(max_threads);
        let session = self.load_id.load(Ordering::SeqCst);

        for idx in visible_start..=visible_end {
            if let Some(wrapper) = self.tabs[self.active_tab_index].files.get(idx) {
                let item = wrapper.borrow();
                if !item.is_dir && item.thumbnail.is_none() && self.pending_thumbnails.insert(idx) {
                    let (is_img, is_vid) = utils::media::is_visual_media(&item.path);
                    if is_img || is_vid {
                        let token = self.thumbnail_manager.register_token(idx);
                        self.spawn_single_thumbnail(
                            idx,
                            item.path.clone(),
                            session,
                            self.active_tab_index,
                            token,
                            sem.clone(),
                            sender.clone(),
                        );
                    } else {
                        self.pending_thumbnails.remove(&idx);
                    }
                }
            }
        }
    }

    /// Receives generated thumbnail textures and updates grid items.
    pub fn handle_thumbnail_ready(
        &mut self,
        grid_idx: u32,
        texture: gdk::Texture,
        load_id: u64,
        tab_index: usize,
    ) {
        crate::hit!("handle_thumbnail_ready");
        if load_id != self.load_id.load(Ordering::SeqCst) {
            return;
        }
        let tab = match self.tabs.get_mut(tab_index) {
            Some(t) => t,
            None => return,
        };

        if grid_idx >= tab.files.len() {
            return;
        }

        let matches = tab
            .files
            .get(grid_idx)
            .is_some_and(|w| w.borrow().grid_idx == grid_idx);
        if !matches {
            debug_assert!(false, "grid_idx {grid_idx} does not match its slot");
            return;
        }

        if let Some(wrapper) = tab.files.get(grid_idx) {
            let mut item = wrapper.borrow().clone();
            item.thumbnail = Some(texture.clone());
            let path = item.path.clone();

            tab.files.remove(grid_idx);
            tab.files.insert(grid_idx, item);

            let cache_key = self
                .current_path
                .canonicalize()
                .unwrap_or_else(|_| self.current_path.clone());

            if let Some(cached) = self.folder_cache.get_mut(&cache_key) {
                cached.thumbnails.insert(path, texture);
            }
        }
    }

    pub fn check_visible_thumbnails(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("check_visible_thumbnails");
        if !self.config.ui.lazy_thumbnails {
            return;
        }

        let total_items = match self.tabs.get(self.active_tab_index) {
            Some(t) if !t.files.is_empty() => t.files.len() as usize,
            _ => return,
        };

        let current_session = self.load_id.load(std::sync::atomic::Ordering::SeqCst);

        let vadj = self.tabs[self.active_tab_index].files.view.vadjustment();
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

        let last_idx = self.tabs[self.active_tab_index].last_thumb_scroll_idx;
        self.tabs[self.active_tab_index].last_thumb_scroll_idx = current_idx;

        let window_size = 60usize.min(total_items);
        let min_pos = current_idx.min(last_idx).saturating_sub(window_size / 2);
        let max_pos = (current_idx.max(last_idx) + window_size).min(total_items);

        let cancelled = self
            .thumbnail_manager
            .cancel_out_of_viewport(min_pos as u32, max_pos as u32);
        for idx in cancelled {
            self.tabs[self.active_tab_index]
                .pending_thumbnails
                .remove(&idx);
        }

        let max_threads = self.config.ui.thumbnail_threads.max(1);
        let sem = self.thumbnail_manager.get_semaphore(max_threads);

        for i in min_pos..max_pos {
            // Scope the immutable borrow of active_files tightly per iteration
            let item_data = {
                let active_files = &self.tabs[self.active_tab_index].files;
                if let Some(wrapper) = active_files.get(i as u32) {
                    let item = wrapper.borrow();
                    if self.tabs[self.active_tab_index]
                        .pending_thumbnails
                        .contains(&item.grid_idx)
                    {
                        None
                    } else if !item.is_dir && item.thumbnail.is_none() {
                        Some((item.grid_idx, item.path.clone()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some((grid_idx, path)) = item_data {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_default();

                let is_pdf = ext == "pdf";
                let is_font = matches!(ext.as_str(), "ttf" | "otf" | "woff" | "woff2" | "ttc");
                let is_exe = ext == "exe";
                let is_audio = utils::media::is_audio_file(&path);
                let (is_img, is_vid) = if !is_pdf && !is_font && !is_exe && !is_audio {
                    utils::media::is_visual_media(&path)
                } else {
                    (false, false)
                };

                let can_thumbnail = (is_pdf && self.config.ui.thumbnail_types.pdfs)
                    || (is_font && self.config.ui.thumbnail_types.fonts)
                    || (is_exe && self.config.ui.thumbnail_types.executables)
                    || (is_audio && self.config.ui.thumbnail_types.audio)
                    || (is_img && self.config.ui.thumbnail_types.images)
                    || (is_vid && self.config.ui.thumbnail_types.videos);

                if can_thumbnail {
                    self.tabs[self.active_tab_index]
                        .pending_thumbnails
                        .insert(grid_idx);
                    let token = self.thumbnail_manager.register_token(grid_idx);
                    self.spawn_single_thumbnail(
                        grid_idx,
                        path,
                        current_session,
                        self.active_tab_index,
                        token,
                        sem.clone(),
                        sender.clone(),
                    );
                }
            }
        }
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

        let max_threads = self.config.ui.thumbnail_threads.max(1);
        let sem = self.thumbnail_manager.get_semaphore(max_threads);
        let token = self.thumbnail_manager.register_token(grid_idx);

        self.spawn_single_thumbnail(
            grid_idx,
            path,
            current_session,
            self.active_tab_index,
            token,
            sem,
            sender.clone(),
        );
    }
}
