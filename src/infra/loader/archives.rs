use super::media::is_visual_media_by_ext;
use super::pool::loader_pool;
use crate::model::{AppMsg, FluxApp, SortBy};
use crate::services::archive;
use crate::ui::FileItem;
use crate::utils;
use crate::utils::media::is_audio_file;
use adw::prelude::*;
use rayon::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
    /// Populates the file grid with the immediate children of `prefix` inside the
    /// archive located at `archive_path`.
    ///
    /// The virtual current path is set to an `archive://` URI so that breadcrumb
    /// rendering, history management, and the back-button all work identically to
    /// real directory navigation. No files are extracted to disk.
    ///
    /// # Arguments
    /// * `archive_path` - Real on-disk path of the archive file.
    /// * `prefix`       - Inner path component being listed (`""` = root level).
    /// * `sender`       - Component handle for dispatching lifecycle messages.
    pub fn load_archive(
        &mut self,
        archive_path: PathBuf,
        prefix: String,
        mut password: Option<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("load_archive");
        if let Some(old_mon) = self.directory_monitor.take() {
            old_mon.cancel();
        }
        self.files.clear();
        self.is_loading = true;
        // Bump the session counter and capture the resulting ID so the
        // spawned closure can stamp the message it will later dispatch.
        let session_id = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.pending_thumbnails.clear();
        self.thumbnail_manager.clear_and_cancel_all();

        self.current_path = archive::build_archive_uri(&archive_path, &prefix);

        if password.is_none() {
            password = self.cached_archive_password.clone();
        }

        let archive_path_c = archive_path.clone();
        let prefix_c = prefix.clone();
        let password_c = password.clone();
        let sender_c = sender.clone();

        relm4::spawn_blocking(move || {
            let result =
                archive::list_archive_entries(&archive_path_c, &prefix_c, password_c.as_deref());

            sender_c.input(AppMsg::ArchiveLoaded {
                archive_path: archive_path_c,
                prefix: prefix_c,
                password: password_c,
                load_id: session_id,
                result,
            });
        });
    }

    pub fn handle_archive_loaded(
        &mut self,
        archive_path: PathBuf,
        prefix: String,
        password: Option<String>,
        load_id: u64,
        result: Result<Vec<archive::ArchiveEntry>, archive::ArchiveError>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_archive_loaded");
        self.is_loading = false;

        // Discard results from superseded navigation sessions.
        if load_id != self.load_id.load(Ordering::SeqCst) {
            return;
        }

        let current_session = load_id;
        let expand_labels = self.config.ui.expand_labels;
        let sort_strategy = self.sort_by;
        let sort_ascending = self.sort_ascending;
        let folders_first = self.config.ui.folders_first;
        let extension_globset = self.extension_globset.clone();

        match result {
            Err(archive::ArchiveError::PasswordRequired) => {
                self.cached_archive_password = None;
                sender.input(AppMsg::PromptArchivePassword {
                    archive_path,
                    prefix,
                    wrong_password: false,
                });
            }
            Err(archive::ArchiveError::WrongPassword) => {
                self.cached_archive_password = None;
                sender.input(AppMsg::PromptArchivePassword {
                    archive_path,
                    prefix,
                    wrong_password: true,
                });
            }
            Err(archive::ArchiveError::Other(e)) => {
                sender.input(AppMsg::ShowToast(e));
            }
            Ok(entries) => {
                if password.is_some() {
                    self.cached_archive_password = password;
                }

                let mut items =
                    archive::entries_to_load_contexts(&entries, &archive_path, expand_labels);

                if let Some(ref gs) = extension_globset {
                    items.retain(|item| {
                        if item.is_dir {
                            return true;
                        }
                        gs.is_match(&item.sort_name)
                    });
                }

                // Sort entries
                loader_pool().install(|| {
                    items.par_sort_unstable_by(move |a, b| {
                        if a.is_dir != b.is_dir {
                            return if folders_first {
                                b.is_dir.cmp(&a.is_dir)
                            } else {
                                a.is_dir.cmp(&b.is_dir)
                            };
                        }

                        // In directory scan sort comparator:
                        let primary_order = match sort_strategy {
                            SortBy::Name => a.sort_name.cmp(&b.sort_name),
                            SortBy::Size => a.size().cmp(&b.size()),
                            SortBy::Date => a.mtime().cmp(&b.mtime()),
                            SortBy::Type => a.sort_ext.cmp(&b.sort_ext),
                        };

                        let tie = if primary_order == std::cmp::Ordering::Equal {
                            a.sort_name.cmp(&b.sort_name)
                        } else {
                            primary_order
                        };

                        if sort_ascending {
                            tie
                        } else {
                            tie.reverse()
                        }
                    });
                });

                // Clear the active tab's grid before populating virtual archive items
                self.tabs[self.active_tab_index].files.clear();

                let active_tab = &mut self.tabs[self.active_tab_index];
                if self.is_list_mode {
                    active_tab.files.view.set_min_columns(1);
                    active_tab.files.view.set_max_columns(1);
                } else {
                    active_tab.files.view.set_min_columns(1);
                    active_tab.files.view.set_max_columns(20);
                }

                let mut media_tasks: Vec<(u32, PathBuf)> = Vec::new();
                for (grid_idx, item) in (0u32..).zip(items) {
                    let size = item.size();
                    let mtime = item.mtime();
                    let is_empty = if item.is_dir && self.config.ui.show_empty_dir_emblem {
                        item.is_empty()
                    } else {
                        false
                    };

                    let icon = utils::icon::get_icon_for_path(&item.target_path, item.is_dir);

                    // Collect visual media files for thumbnail generation
                    if !item.is_dir {
                        let (is_img, is_vid) = is_visual_media_by_ext(&item.target_path);
                        let is_audio = is_audio_file(&item.target_path);
                        if is_img || is_vid || is_audio {
                            media_tasks.push((grid_idx, item.target_path.clone()));
                        }
                    }

                    let display_label = crate::utils::helpers::format_display_label(
                        &item.display_name,
                        item.is_dir,
                        &self.config.ui.hidden_extensions,
                    );

                    self.tabs[self.active_tab_index].files.append(
                        FileItem::builder(item.display_name.clone(), item.target_path, icon)
                            .display_label(display_label)
                            .is_dir(item.is_dir)
                            .size(size)
                            .mtime(mtime)
                            .is_empty(is_empty)
                            .expand_labels(item.expand_labels)
                            .icon_size(if self.is_list_mode {
                                self.current_list_icon_size
                            } else {
                                self.current_icon_size
                            })
                            .is_list_mode(self.is_list_mode)
                            .grid_idx(grid_idx)
                            .max_width_chars(self.config.ui.max_width_chars)
                            .grid_spacing(self.config.ui.grid_spacing)
                            .show_symlink_emblem(self.config.ui.show_symlink_emblem)
                            .scale_font_with_icons(self.config.ui.scale_font_with_icons)
                            .default_icon_size(self.config.ui.default_icon_size)
                            .show_empty_dir_emblem(self.config.ui.show_empty_dir_emblem)
                            .disable_drag_and_drop(self.config.ui.disable_drag_and_drop)
                            .build(),
                    );
                }

                self.update_breadcrumbs();

                if !self.config.ui.lazy_thumbnails {
                    self.spawn_thumbnail_loader(
                        media_tasks,
                        current_session,
                        self.active_tab_index,
                        sender.clone(),
                    );
                } else {
                    sender.input(AppMsg::CheckVisibleThumbnails);
                }
            }
        }
    }
}
