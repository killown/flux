use super::icons::get_custom_extension_icon_path;
use super::media::is_visual_media_by_ext;
use crate::model::{AppMsg, FileLoadContext, FluxApp};
use crate::ui::FileItem;
use crate::utils;
use crate::utils::media::is_audio_file;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn handle_show_loading_spinner(&mut self, session: u64) {
        if self.load_id.load(Ordering::SeqCst) == session {
            self.is_loading = true;
        }
    }

    pub fn handle_folder_loaded_chunk(
        &mut self,
        load_id: u64,
        chunk: Vec<FileLoadContext>,
        is_cached: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_folder_loaded_chunk");
        if self.load_id.load(Ordering::SeqCst) == load_id {
            self.append_context_batch(chunk, load_id, is_cached, sender);
        }
    }

    pub fn handle_folder_loaded_finish(&mut self, load_id: u64) {
        if self.load_id.load(Ordering::SeqCst) == load_id {
            self.is_loading = false;
        }
    }

    pub fn handle_folder_loaded_begin(
        &mut self,
        path: PathBuf,
        load_id: u64,
        items: Vec<FileLoadContext>,
        media_tasks: Vec<(u32, PathBuf)>,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.active_video_preview = None;
        self.handle_folder_loaded(path, load_id, items, media_tasks, sender);
    }

    pub fn handle_invalidate_cache_and_navigate(
        &mut self,
        path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        if let Some(parent) = path.parent() {
            self.folder_cache.remove(&self.cache_key(parent));
        }
        self.folder_cache
            .remove(&self.cache_key(&self.current_path));
        sender.input(AppMsg::Navigate(path));
    }

    /// construct and append a slice of FileLoadContext items directly to self.files.
    pub fn append_context_batch(
        &mut self,
        items: Vec<FileLoadContext>,
        load_id: u64,
        _is_cached: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("append_context_batch");
        let max_width_chars = self.config.ui.max_width_chars;
        let grid_spacing = self.config.ui.grid_spacing;
        let is_list_mode = self.is_list_mode;
        let list_icon_size = self.current_list_icon_size;
        let grid_icon_size = self.current_icon_size;
        let config_file_icons = &self.config.ui.file_icons;
        let config_folder_icons = &self.config.ui.folder_icons;

        let cache_key = self
            .current_path
            .canonicalize()
            .unwrap_or_else(|_| self.current_path.clone());
        let cached_thumbs = self.folder_cache.get(&cache_key).map(|c| &c.thumbnails);

        let active_tab = &mut self.tabs[self.active_tab_index];
        let start_idx = active_tab.files.len();
        let mut chunk_media_tasks: Vec<(u32, PathBuf)> = Vec::new();

        let git_map = (self.git_status_dir == self.current_path).then_some(&self.git_status_map);
        for (offset, item) in items.into_iter().enumerate() {
            let grid_idx = start_idx + offset as u32;
            let current_uid = unsafe { libc::geteuid() };
            let size = item.size();
            let mtime = item.mtime();
            let is_foreign_owner = item.is_foreign_owner(current_uid);
            let is_empty = if item.is_dir && self.config.ui.show_empty_dir_emblem {
                item.is_empty()
            } else {
                false
            };

            let custom_icon = config_file_icons
                .get(&item.target_path.to_string_lossy().to_string())
                .cloned()
                .or_else(|| {
                    if item.is_dir {
                        config_folder_icons
                            .get(&item.target_path.to_string_lossy().to_string())
                            .cloned()
                    } else if !item.sort_ext.is_empty() {
                        get_custom_extension_icon_path(&item.sort_ext)
                            .map(|p| p.to_string_lossy().into_owned())
                    } else {
                        None
                    }
                })
                .or_else(|| item.custom_icon.clone());

            let icon = if let Some(ref custom) = custom_icon {
                gtk::gio::Icon::for_string(custom).unwrap_or_else(|_| {
                    utils::icon::get_icon_for_path(&item.target_path, item.is_dir)
                })
            } else {
                utils::icon::get_icon_for_path(&item.target_path, item.is_dir)
            };

            let thumbnail = cached_thumbs
                .and_then(|m| m.get(&item.target_path))
                .cloned();

            let is_inside_thumb_cache = dirs::cache_dir()
                .map(|c| item.target_path.starts_with(c.join("thumbnails")))
                .unwrap_or(false);

            if !item.is_dir && thumbnail.is_none() && !is_inside_thumb_cache {
                let (is_img, is_vid) = is_visual_media_by_ext(&item.target_path);
                let is_exe = item
                    .target_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
                let is_audio = is_audio_file(&item.target_path);

                if is_img || is_vid || is_exe || is_audio {
                    let source = custom_icon
                        .as_ref()
                        .map(PathBuf::from)
                        .or_else(|| item.thumbnail_path.clone())
                        .unwrap_or_else(|| item.target_path.clone());
                    chunk_media_tasks.push((grid_idx, source));
                }
            }

            let git_status = git_map
                .and_then(|m| m.get(&item.target_path))
                .copied()
                .unwrap_or_default();

            let display_label = crate::utils::helpers::format_display_label(
                &item.display_name,
                item.is_dir,
                &self.config.ui.hidden_extensions,
            );

            let file_item = FileItem::builder(item.display_name, item.target_path, icon)
                .display_label(display_label)
                .is_dir(item.is_dir)
                .thumbnail(thumbnail)
                .icon_size(if is_list_mode {
                    list_icon_size
                } else {
                    grid_icon_size
                })
                .size(size)
                .mtime(mtime)
                .is_foreign_owner(is_foreign_owner)
                .is_empty(is_empty)
                .expand_labels(item.expand_labels)
                .is_list_mode(is_list_mode)
                .is_custom_icon(custom_icon.is_some())
                .grid_idx(grid_idx)
                .max_width_chars(max_width_chars)
                .grid_spacing(grid_spacing)
                .is_symlink(item.is_symlink)
                .is_broken_symlink(item.is_broken_symlink)
                .show_symlink_emblem(self.config.ui.show_symlink_emblem)
                .git_status(git_status)
                .scale_font_with_icons(self.config.ui.scale_font_with_icons)
                .default_icon_size(self.config.ui.default_icon_size)
                .show_empty_dir_emblem(self.config.ui.show_empty_dir_emblem)
                .disable_drag_and_drop(self.config.ui.disable_drag_and_drop)
                .build();

            self.tabs[self.active_tab_index].files.append(file_item);
        }

        if !chunk_media_tasks.is_empty() {
            if !self.config.ui.lazy_thumbnails {
                self.spawn_thumbnail_loader(
                    chunk_media_tasks,
                    load_id,
                    self.active_tab_index,
                    sender.clone(),
                );
            } else {
                sender.input(AppMsg::CheckVisibleThumbnails);
            }
        }
    }

    /// Streams background folder results into the grid in batches.
    /// Discards stale sessions by checking `load_id`.
    pub fn handle_folder_loaded(
        &mut self,
        path: PathBuf,
        load_id: u64,
        items: Vec<FileLoadContext>,
        media_tasks: Vec<(u32, PathBuf)>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_folder_loaded");
        if load_id != self.load_id.load(Ordering::SeqCst) {
            return;
        }

        // WARNING: changing this could cause some bugs:
        // Cancels the in-flight ShowLoadingSpinner timer (its session check will now fail)
        // and rebinds load_id so all downstream chunk/finish guards still match self.load_id.
        // The +1 is mandatory: fetch_add returns the old value.
        let load_id = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;

        self.is_loading = false;
        self.archive_locked = false;

        let cache_cap = self.config.ui.folder_cache_capacity;

        // If caching is disabled, drop any previously cached folders immediately.
        if cache_cap == 0 {
            self.folder_cache.clear();
        }

        let cache_key = path.canonicalize().unwrap_or_else(|_| path.clone());
        let is_cached = self.folder_cache.contains_key(&cache_key);

        if !path.to_string_lossy().starts_with("trash://")
            && !path
                .to_string_lossy()
                .starts_with(crate::services::archive::ARCHIVE_URI)
            && self.filter.is_empty()
            && self.extension_globset.is_none()
            && !is_cached
            && cache_cap > 0
        {
            if self.folder_cache.len() >= cache_cap {
                if let Some(oldest) = self
                    .folder_cache
                    .iter()
                    .min_by_key(|(_, v)| v.last_visited)
                    .map(|(k, _)| k.clone())
                {
                    self.folder_cache.remove(&oldest);
                }
            }

            self.folder_cache.insert(
                cache_key,
                crate::model::CachedFolder {
                    items: items.clone(),
                    media_tasks,
                    thumbnails: std::collections::HashMap::new(),
                    last_visited: std::time::Instant::now(),
                },
            );
        }

        // CLEAR the grid completely so Relm4 drops all old FileItems
        // and cleans up widget associations, qdata, and MultiSelection state.
        // WARNING: this is necessary for folder cache, if no clear here, it will start mixing files
        // from different folders and cause all sorts of problems.
        self.tabs[self.active_tab_index].files.clear();

        let tab = &mut self.tabs[self.active_tab_index];
        if self.is_list_mode {
            if tab.files.view.max_columns() != 1 {
                tab.files.view.set_min_columns(1);
                tab.files.view.set_max_columns(1);
            }
        } else if tab.files.view.max_columns() != 20 {
            tab.files.view.set_min_columns(1);
            tab.files.view.set_max_columns(20);
        }

        self.current_path = path;
        self.update_breadcrumbs();

        let path_str = self.current_path.to_string_lossy();
        let can_scan_git = !path_str.starts_with("trash://")
            && !path_str.starts_with("recent:///")
            && !path_str.starts_with(crate::services::archive::ARCHIVE_URI)
            && !crate::services::network::is_network_uri(&self.current_path);

        if can_scan_git {
            let target_dir = self.current_path.clone();
            let current_load_id = self.load_id.load(Ordering::SeqCst);
            let s_clone = sender.clone();

            relm4::spawn(async move {
                if let Some(repo_root) = crate::services::git::find_git_repo_root(&target_dir) {
                    s_clone.input(AppMsg::SetGitRepoActive(true));

                    if let Ok(raw_status) =
                        crate::services::git::query_git_status_for_view(&repo_root, &target_dir)
                            .await
                    {
                        s_clone.input(AppMsg::GitStatusReady {
                            path: target_dir,
                            load_id: current_load_id,
                            updates: raw_status,
                        });
                    }
                } else {
                    s_clone.input(AppMsg::SetGitRepoActive(false));
                }
            });
        } else {
            self.is_in_git_repo = false;
        }

        // Sync active tab so SwitchTab restores the correct path
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.current_path = self.current_path.clone();
            let title = self
                .current_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| self.current_path.to_string_lossy().to_string());
            tab.title = title.clone();
            if (self.active_tab_index as i32) < self.tab_view.n_pages() {
                let page = self.tab_view.nth_page(self.active_tab_index as i32);
                page.set_title(&title);
            }
        }

        let batch_size = self.config.ui.loader_batch_size.max(10);

        if items.len() <= batch_size {
            self.append_context_batch(items, load_id, is_cached, sender);
        } else {
            let mut remaining = items;
            let first_batch: Vec<FileLoadContext> = remaining.drain(..batch_size).collect();
            // Append and immediately trigger thumbnails for the initial visible batch
            self.append_context_batch(first_batch, load_id, is_cached, sender);

            let session_arc = self.load_id.clone();
            let sender_clone = sender.clone();
            let mut chunks: Option<Vec<Vec<FileLoadContext>>> = {
                let mut v: Vec<Vec<FileLoadContext>> =
                    remaining.chunks(batch_size).map(|c| c.to_vec()).collect();
                v.reverse();
                Some(v)
            };

            glib::idle_add_local(move || {
                if session_arc.load(Ordering::SeqCst) != load_id {
                    chunks.take(); // free all pending item data immediately
                    return glib::ControlFlow::Break;
                }

                if let Some(chunk) = chunks.as_mut().and_then(|v| v.pop()) {
                    sender_clone.input(AppMsg::FolderLoadedChunk {
                        load_id,
                        chunk,
                        is_cached,
                    });

                    if chunks.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
                        sender_clone.input(AppMsg::FolderLoadedFinish { load_id });
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    }
                } else {
                    glib::ControlFlow::Break
                }
            });
        }

        if let Some(selection_model) = self.tabs[self.active_tab_index]
            .files
            .view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
        {
            selection_model.unselect_all();
        }
        self.selection_status.clear();
    }
}
