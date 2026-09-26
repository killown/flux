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

        let active_files = match self.tabs.get(self.active_tab_index) {
            Some(t) => &t.files,
            None => return,
        };

        if let Some(selection_model) = active_files
            .view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
        {
            let selection = selection_model.selection();
            let n_selected = selection.size();

            for i in 0..n_selected {
                let pos = selection.nth(i as u32);
                if let Some(item_wrapper) = active_files.get(pos) {
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
                let selected_item = active_files
                    .view
                    .model()
                    .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                    .and_then(|m| {
                        let pos = m.selection().nth(0);
                        active_files.get(pos).map(|w| w.borrow().clone())
                    });

                if let Some(ref item) = selected_item {
                    let path = item.path.clone();
                    let s = sender.clone();

                    // Resolve symlink target if applicable, showing full path instead of filename name repetition
                    let display_name = if let Ok(meta) = std::fs::symlink_metadata(&path) {
                        if meta.is_symlink() {
                            path.canonicalize()
                                .map(|real| real.display().to_string())
                                .unwrap_or_else(|_| item.name.clone())
                        } else {
                            item.name.clone()
                        }
                    } else {
                        item.name.clone()
                    };

                    // Spawn async tasks for MIME, dimensions, media duration
                    let path_for_async = path.clone();
                    relm4::spawn_blocking(move || {
                        let mime = utils::get_mime_type(&path_for_async);
                        let dimensions = if mime.starts_with("image/") {
                            crate::utils::media::probe_image_dimensions(&path_for_async)
                        } else {
                            None
                        };

                        if mime.starts_with("audio/") || mime.starts_with("video/") {
                            let path_c = path_for_async.clone();
                            let s_c = s.clone();
                            relm4::spawn(async move {
                                let dur = crate::utils::media::probe_media_duration(&path_c).await;
                                s_c.input(AppMsg::MediaDurationReady(dur));
                            });
                        }

                        s.input(AppMsg::FileMetaReady { mime, dimensions });
                    });

                    // Build the base status string (name/path, size, date)
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
                        format!("{} ({}) · {}", display_name, size_str, dt_formatted)
                    } else {
                        format!("{} ({})", display_name, size_str)
                    }
                } else {
                    format!("{} ({})", single_name, size_str)
                }
            }

            // Single folder
            (1, _, true) => {
                let item = active_files
                    .view
                    .model()
                    .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                    .and_then(|m| {
                        let pos = m.selection().nth(0);
                        active_files.get(pos)
                    });
                if let Some(wrapper) = item {
                    let borrowed = wrapper.borrow();
                    let path = borrowed.path.clone();
                    let real_path = path.canonicalize().unwrap_or_else(|_| path.clone());
                    let child_count = std::fs::read_dir(&real_path)
                        .map(|rd| rd.count())
                        .unwrap_or(0);

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

                    let path_display = real_path.display().to_string();

                    let status = if let Some(dt_formatted) = date_str {
                        format!(
                            "{} ({} items) · {}",
                            path_display, child_count, dt_formatted
                        )
                    } else {
                        format!("{} ({} items)", path_display, child_count)
                    };

                    status
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

        self.sync_video_preview();
    }

    /// Toggles between grid card layout and compact list view.
    pub fn handle_toggle_list_mode(&mut self) {
        self.is_list_mode = !self.is_list_mode;
        self.saved_list_mode = self.is_list_mode;
        self.config.default_list_mode = self.is_list_mode;
        utils::save_config(&self.config);

        let is_list = self.is_list_mode;
        let new_icon_size = if is_list {
            self.current_list_icon_size
        } else {
            self.current_icon_size
        };

        let active_tab = match self.tabs.get_mut(self.active_tab_index) {
            Some(t) => t,
            None => return,
        };

        if is_list {
            active_tab.files.view.set_min_columns(1);
            active_tab.files.view.set_max_columns(1);
        } else {
            active_tab.files.view.set_min_columns(1);
            active_tab.files.view.set_max_columns(20);
        }

        for i in 0..active_tab.files.len() {
            if let Some(item_wrapper) = active_tab.files.get(i) {
                let mut item = item_wrapper.borrow().clone();
                item.is_list_mode = is_list;
                item.icon_size = new_icon_size;
                active_tab.files.remove(i);
                active_tab.files.insert(i, item);
            }
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

            let active_files = &mut self.tabs[self.active_tab_index].files;
            for i in 0..active_files.len() {
                if let Some(item_wrapper) = active_files.get(i) {
                    if item_wrapper.borrow().is_list_mode {
                        let mut item = item_wrapper.borrow().clone();
                        item.icon_size = new_size;
                        active_files.remove(i);
                        active_files.insert(i, item);
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

            let active_files = &mut self.tabs[self.active_tab_index].files;
            for i in 0..active_files.len() {
                if let Some(item_wrapper) = active_files.get(i) {
                    if !item_wrapper.borrow().is_list_mode {
                        let mut item = item_wrapper.borrow().clone();
                        item.icon_size = new_size;
                        active_files.remove(i);
                        active_files.insert(i, item);
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
        if load_id != self.load_id.load(Ordering::SeqCst) {
            return;
        }
        let tab = match self.tabs.get_mut(tab_index) {
            Some(t) => t,
            None => return,
        };

        if let Some(pos) = (0..tab.files.len()).find(|&i| {
            tab.files
                .get(i)
                .map(|w| w.borrow().grid_idx == grid_idx)
                .unwrap_or(false)
        }) {
            if let Some(wrapper) = tab.files.get(pos) {
                let mut item = wrapper.borrow().clone();
                item.thumbnail = Some(texture.clone());
                let path = item.path.clone();

                tab.files.remove(pos);
                tab.files.insert(pos, item);

                let cache_key = self
                    .current_path
                    .canonicalize()
                    .unwrap_or_else(|_| self.current_path.clone());

                if let Some(cached) = self.folder_cache.get_mut(&cache_key) {
                    cached.thumbnails.insert(path, texture);
                }
            }
        }
    }

    pub fn check_visible_thumbnails(&mut self, sender: &AsyncComponentSender<Self>) {
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
                let is_audio = utils::is_audio_file(&path);
                let (is_img, is_vid) = if !is_pdf && !is_font && !is_exe && !is_audio {
                    utils::is_visual_media(&path)
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

        // Avoid duplicate appending if the MIME type is already part of the status string
        if self.selection_status.contains(&mime) {
            return;
        }

        let dim_str = dimensions.map(|(w, h)| {
            let ratio = crate::utils::media::aspect_ratio_label(w, h);
            format!(" - {}×{} ({})", w, h, ratio)
        });
        if let Some(d) = dim_str {
            // Avoid duplicate dimensions string if already present
            if !self.selection_status.contains(&d) {
                self.selection_status.push_str(&d);
            }
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

    /// Locates the rendered child widget in the grid view matching the given file path.
    pub fn find_widget_by_path(&self, path: &std::path::Path) -> Option<gtk::Widget> {
        let name = path.to_string_lossy();

        fn search(widget: &gtk::Widget, target: &str) -> Option<gtk::Widget> {
            if widget.widget_name().as_str() == target {
                return Some(widget.clone());
            }
            let mut child = widget.first_child();
            while let Some(c) = child {
                if let Some(found) = search(&c, target) {
                    return Some(found);
                }
                child = c.next_sibling();
            }
            None
        }

        let active_view = self.tabs.get(self.active_tab_index).map(|t| &t.files.view);
        active_view.and_then(|v| search(v.as_ref(), name.as_ref()))
    }

    pub fn stop_video_preview(&mut self) {
        if let Some(source_id) = self.video_preview_source.take() {
            source_id.remove();
        }

        let Some(active_path) = self.active_video_preview.take() else {
            return;
        };

        if let Some(widget) = self.find_widget_by_path(&active_path) {
            unsafe {
                if let Some(video_ptr) = widget.data::<gtk::Video>("video_widget") {
                    let video = video_ptr.as_ref();
                    if let Some(stream) = video.media_stream() {
                        stream.pause();
                    }
                    video.set_media_stream(None::<&gtk::MediaStream>);
                }
                if let Some(stack_ptr) = widget.data::<gtk::Stack>("preview_stack") {
                    stack_ptr.as_ref().set_visible_child_name("icon");
                }
            }
        }
    }

    pub fn sync_video_preview(&mut self) {
        if !self.config.ui.autoplay_video_previews {
            self.stop_video_preview();
            return;
        }

        let selection = self.get_selection();
        if selection.len() != 1 {
            self.stop_video_preview();
            return;
        }

        let selected_path = selection[0].clone();
        let (_, is_vid) = crate::utils::is_visual_media(&selected_path);
        if !is_vid {
            self.stop_video_preview();
            return;
        }

        if self.active_video_preview.as_ref() == Some(&selected_path) {
            return;
        }

        self.stop_video_preview();

        if self.find_widget_by_path(&selected_path).is_some() {
            self.handle_trigger_video_preview(selected_path);
        } else {
            let target_path = selected_path;
            let source_id =
                glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
                    if let Some(s) = crate::model::SENDER.get() {
                        let _ = s.send(AppMsg::TriggerVideoPreview(target_path));
                    }
                });
            self.video_preview_source = Some(source_id);
        }
    }

    pub fn handle_trigger_video_preview(&mut self, path: std::path::PathBuf) {
        self.video_preview_source = None;

        let selection = self.get_selection();
        if selection.len() != 1 || selection[0] != path {
            return;
        }

        // Ensure any active preview is fully torn down first
        self.stop_video_preview();

        self.active_video_preview = Some(path.clone());

        // Resolve real disk path: if it's an archive virtual path, extract to a tempfile first
        let resolved_file = if let Some((archive_path, inner_path)) =
            crate::services::archive::parse_archive_uri(&path.to_string_lossy())
        {
            match crate::services::archive::extract_entry_to_tempfile(
                &archive_path,
                &inner_path,
                None,
            ) {
                Ok(tmp) => {
                    let tmp_path = tmp.path().to_path_buf();
                    // Keep the temp file alive until preview stops or app exits
                    if let Ok(file) = tmp.keep() {
                        let (_, path_buf) = file;
                        Some(gtk::gio::File::for_path(path_buf))
                    } else {
                        Some(gtk::gio::File::for_path(tmp_path))
                    }
                }
                Err(_) => None,
            }
        } else {
            Some(gtk::gio::File::for_path(&path))
        };

        let Some(gfile) = resolved_file else {
            return;
        };

        if let Some(child) = self.find_widget_by_path(&path) {
            let media_file = gtk::MediaFile::for_file(&gfile);
            media_file.set_muted(true);
            media_file.set_loop(true);
            media_file.play();

            unsafe {
                if let Some(video_ptr) = child.data::<gtk::Video>("video_widget") {
                    let video = video_ptr.as_ref();
                    if let Some(old_stream) = video.media_stream() {
                        old_stream.pause();
                    }
                    video.set_autoplay(true);
                    video.set_loop(true);
                    video.set_media_stream(Some(&media_file));
                }
                if let Some(stack_ptr) = child.data::<gtk::Stack>("preview_stack") {
                    stack_ptr.as_ref().set_visible_child_name("video");
                }
            }
        }
    }
}
