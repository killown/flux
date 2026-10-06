use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use crate::utils;
use adw::gio::prelude::*;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;

impl FluxApp {
    /// Updates grid selection metadata and formats the status bar label.
    pub fn handle_selection_changed(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_selection_changed");
        if self.task_queue.summary().is_some() {
            return;
        }

        let mut total_size = 0u64;
        let mut count = 0usize;
        let mut dir_count = 0usize;
        let mut only_files = true;
        let mut only_dirs = true;
        let mut single_name = String::new();
        let mut single_item: Option<crate::ui::FileItem> = None;

        let active_files = match self.tabs.get(self.active_tab_index) {
            Some(t) => &t.files,
            None => return,
        };

        if let Some(selection_model) = active_files
            .view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
        {
            let bitset = selection_model.selection();

            // Collect the visual (filtered-view) indices from the bitset.
            let mut visual_indices: Vec<u32> = Vec::new();
            if let Some((mut iter, first_idx)) = gtk::BitsetIter::init_first(&bitset) {
                let mut current = Some(first_idx);
                while let Some(idx) = current {
                    visual_indices.push(idx);
                    current = iter.next();
                }
            }

            // WARNING: active_files.get(i) always indexes the raw unfiltered store, so passing
            // a filtered-view position directly would return the wrong item.
            let query_lc = self.filter.to_lowercase();

            let resolved: Vec<_> = if self.filter.is_empty() || self.is_content_searching {
                // No filter: visual pos == store pos.
                visual_indices
                    .iter()
                    .filter_map(|&idx| active_files.get(idx))
                    .map(|w| w.borrow().clone())
                    .collect()
            } else if let Some((tags, rest)) = crate::utils::search::parse_tag_filter(&query_lc) {
                let rest_clean = rest.trim().to_lowercase();
                let target_tags: Vec<String> = tags.into_iter().map(|t| t.to_lowercase()).collect();
                let mut match_count = 0u32;
                let mut out = Vec::new();
                for i in 0..active_files.len() {
                    if let Some(wrapper) = active_files.get(i) {
                        let item = wrapper.borrow();
                        let name_ok =
                            rest_clean.is_empty() || item.name.to_lowercase().contains(&rest_clean);
                        if name_ok {
                            let file_tags = crate::utils::xattr::read_tags(&item.path);
                            let file_tags_lc: Vec<String> =
                                file_tags.into_iter().map(|t| t.to_lowercase()).collect();
                            if target_tags.iter().all(|req| file_tags_lc.contains(req)) {
                                if visual_indices.contains(&match_count) {
                                    out.push(item.clone());
                                }
                                match_count += 1;
                            }
                        }
                    }
                }
                out
            } else {
                // Normal fuzzy / size / extension filter.
                let mut match_count = 0u32;
                let mut out = Vec::new();
                for i in 0..active_files.len() {
                    if let Some(wrapper) = active_files.get(i) {
                        let item = wrapper.borrow();
                        if crate::utils::search::fuzzy_match(&item.name, &query_lc) {
                            if visual_indices.contains(&match_count) {
                                out.push(item.clone());
                            }
                            match_count += 1;
                        }
                    }
                }
                out
            };

            for item in &resolved {
                if item.is_dir {
                    only_files = false;
                    dir_count += 1;
                    if count + dir_count == 1 {
                        single_name = item.name.clone();
                        single_item = Some(item.clone());
                    }
                } else {
                    only_dirs = false;
                    total_size += item.size;
                    count += 1;
                    if count + dir_count == 1 {
                        single_name = item.name.clone();
                        single_item = Some(item.clone());
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
                if let Some(ref item) = single_item {
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
                        let mime = utils::media::get_mime_type(&path_for_async);
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
                if let Some(borrowed) = single_item.as_ref() {
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

        if self.config.ui.auto_show_diff && self.is_in_git_repo {
            let mut diff_triggered = false;

            if let Some(item) = single_item.as_ref() {
                if !item.is_dir {
                    let target_path = &item.path;
                    let has_diff = self
                        .git_status_map
                        .get(target_path)
                        .or_else(|| {
                            let canon = target_path.canonicalize().ok()?;
                            self.git_status_map.get(&canon)
                        })
                        .is_some_and(|&s| {
                            s != crate::services::git::GitFileStatus::None
                                && s != crate::services::git::GitFileStatus::Ignored
                        });

                    if has_diff {
                        diff_triggered = true;
                        sender.input(AppMsg::ShowFileDiff(target_path.clone()));
                    }
                }
            }

            if !diff_triggered && self.diff_panel_visible {
                self.toggle_sidebar_right_panel(crate::model::RightPanelType::Diff, sender);
            }
        }

        self.sync_video_preview();
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
}
