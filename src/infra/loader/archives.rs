use super::media::is_visual_media_by_ext;
use super::pool::loader_pool;
use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, ArchiveMsg, SortBy, ThumbnailsMsg, WindowMsg};
use crate::domain::model::{ArchiveEntry, ArchiveError};
use crate::infra::archive;
use crate::ui::FileItem;
use crate::utils;
use crate::utils::media::is_audio_file;
use adw::prelude::*;
use rayon::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

/// Runs the blocking archive listing on the blocking pool and awaits it.
async fn list_off_thread(
    archive_path: &std::path::Path,
    prefix: &str,
    password: Option<String>,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    crate::hit!("list_off_thread");
    let archive_path = archive_path.to_path_buf();
    let prefix = prefix.to_string();
    match relm4::spawn_blocking(move || {
        archive::list_archive_entries(&archive_path, &prefix, password.as_deref())
    })
    .await
    {
        Ok(result) => result,
        Err(e) => Err(ArchiveError::Other(format!(
            "archive listing task failed: {e}"
        ))),
    }
}

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
    /// * `password`     - Password the caller already has (typed by the user). When
    ///   `None`, the session map is consulted, then the system keyring.
    /// * `sender`       - Component handle for dispatching lifecycle messages.
    pub fn load_archive(
        &mut self,
        archive_path: PathBuf,
        prefix: String,
        password: Option<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("load_archive");
        if let Some(old_mon) = self.directory_monitor.take() {
            old_mon.cancel();
        }
        self.files.clear();
        self.is_loading = true;
        let session_id = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.pending_thumbnails.clear();
        self.thumbnail_manager.clear_and_cancel_all();

        self.current_path = archive::build_archive_uri(&archive_path, &prefix);

        let known_password =
            password.or_else(|| self.archive.passwords.get(&archive_path).cloned());

        let archive_path_c = archive_path.clone();
        let prefix_c = prefix.clone();
        let sender_c = sender.clone();

        relm4::spawn(async move {
            let mut password_c = known_password;
            let mut result = list_off_thread(&archive_path_c, &prefix_c, password_c.clone()).await;

            let needs_password = matches!(result, Err(ArchiveError::PasswordRequired));
            if password_c.is_none() && needs_password {
                if let Some(saved) =
                    crate::infra::keyring::lookup_archive_password(&archive_path_c).await
                {
                    result = list_off_thread(&archive_path_c, &prefix_c, Some(saved.clone())).await;
                    password_c = Some(saved);
                }
            }

            sender_c.input(AppMsg::Archive(ArchiveMsg::ArchiveLoaded {
                archive_path: archive_path_c,
                prefix: prefix_c,
                password: password_c,
                load_id: session_id,
                result,
            }));
        });
    }

    /// Paths-only copy of the session's archive lock state for the loader pool.
    pub fn archive_lock_snapshot(&self) -> archive::ArchiveLockSnapshot {
        archive::ArchiveLockSnapshot::new(
            self.archive.known_encrypted.clone(),
            self.archive.passwords.keys().cloned().collect(),
        )
    }

    /// Pushes the session's lock state for `archive_path` into every grid card that
    /// shows it, so the padlock flips without a reload.
    pub fn sync_archive_lock_badges(&mut self, archive_path: &std::path::Path) {
        crate::hit!("sync_archive_lock_badges");
        let encrypted = self.archive.known_encrypted.contains(archive_path);
        let unlocked = self.archive.passwords.contains_key(archive_path);

        for tab in &mut self.tabs {
            let selection = tab.files.view.model().and_downcast::<gtk::MultiSelection>();
            for idx in 0..tab.files.len() {
                let Some(wrapper) = tab.files.get(idx) else {
                    continue;
                };
                let next = {
                    let item = wrapper.borrow();
                    if item.path.as_path() != archive_path {
                        continue;
                    }
                    let is_encrypted = item.is_encrypted_archive || encrypted;
                    if is_encrypted == item.is_encrypted_archive
                        && unlocked == item.has_cached_password
                    {
                        continue;
                    }
                    let mut next = item.clone();
                    next.is_encrypted_archive = is_encrypted;
                    next.has_cached_password = unlocked;
                    next
                };
                drop(wrapper);

                let was_selected = selection.as_ref().is_some_and(|m| m.is_selected(idx));
                tab.files.remove(idx);
                tab.files.insert(idx, next);
                if was_selected {
                    if let Some(m) = &selection {
                        m.select_item(idx, false);
                    }
                }
            }
        }
    }

    /// Applies the result of the background ZIP header probe. The session set is
    /// updated and every card showing a probed archive is refreshed.
    pub fn handle_archive_locks_probed(&mut self, _load_id: u64, encrypted: Vec<PathBuf>) {
        crate::hit!("handle_archive_locks_probed");
        for path in encrypted {
            if self.archive.known_encrypted.insert(path.clone()) {
                self.sync_archive_lock_badges(&path);
            }
        }
    }

    /// Queues a keyring save requested from the password dialog.
    pub fn queue_archive_password_save(&mut self, archive_path: PathBuf, password: String) {
        crate::hit!("queue_archive_password_save");
        self.archive
            .pending_keyring_saves
            .queue(archive_path, password);
    }

    /// Persists `password` for `archive_path` in the system keyring.
    pub fn handle_save_archive_password_to_keyring(
        &self,
        archive_path: PathBuf,
        password: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_save_archive_password_to_keyring");
        let sender_c = sender.clone();
        relm4::spawn(async move {
            let saved =
                crate::infra::keyring::save_archive_password(&archive_path, &password).await;
            let toast = if saved {
                crate::i18n::tr("Password saved to the system keyring")
            } else {
                crate::i18n::tr("Could not save the password to the system keyring")
            };
            sender_c.input(AppMsg::Window(WindowMsg::ShowToast(toast)));
        });
    }

    /// Deletes the saved keyring password for the archive behind `target` and
    /// drops the session copy, so the next open prompts again. A passphrase
    /// saved for `target` as a LUKS image is deleted as well.
    pub fn handle_forget_archive_password(
        &mut self,
        target: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_forget_archive_password");
        let target_str = target.to_string_lossy().into_owned();
        let archive_path = match archive::parse_archive_uri(&target_str) {
            Some((archive_path, _)) => archive_path,
            None => target,
        };
        self.archive.passwords.remove(&archive_path);
        self.sync_archive_lock_badges(&archive_path);

        let sender_c = sender.clone();
        relm4::spawn(async move {
            let archive_forgotten =
                crate::infra::keyring::forget_archive_password(&archive_path).await;
            let luks_forgotten = crate::infra::keyring::forget_luks_passphrase(&archive_path).await;
            let forgotten = archive_forgotten && luks_forgotten;
            let toast = if forgotten {
                crate::i18n::tr("Saved archive password removed")
            } else {
                crate::i18n::tr("Could not remove the saved archive password")
            };
            sender_c.input(AppMsg::Window(WindowMsg::ShowToast(toast)));
        });
    }

    pub fn handle_archive_loaded(
        &mut self,
        archive_path: PathBuf,
        prefix: String,
        password: Option<String>,
        load_id: u64,
        result: Result<Vec<ArchiveEntry>, ArchiveError>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_archive_loaded");
        self.is_loading = false;

        if load_id != self.load_id.load(Ordering::SeqCst) {
            return;
        }

        let current_session = load_id;
        let expand_labels = self.config.ui.expand_labels;
        let sort_strategy = self.sort_by;
        let sort_ascending = self.sort_ascending;
        let folders_first = self.config.ui.folders_first;
        let archives_as_folders = self.config.ui.archives_as_folders;
        let extension_globset = self.filters.extension_globset.clone();

        match result {
            Err(ArchiveError::PasswordRequired) => {
                self.archive.passwords.remove(&archive_path);
                self.archive.pending_keyring_saves.discard(&archive_path);
                self.archive.known_encrypted.insert(archive_path.clone());
                self.sync_archive_lock_badges(&archive_path);
                sender.input(AppMsg::Archive(ArchiveMsg::PromptArchivePassword {
                    archive_path,
                    prefix,
                    wrong_password: false,
                }));
            }
            Err(ArchiveError::WrongPassword) => {
                self.archive.passwords.remove(&archive_path);
                self.archive.pending_keyring_saves.discard(&archive_path);
                self.archive.known_encrypted.insert(archive_path.clone());
                self.sync_archive_lock_badges(&archive_path);
                sender.input(AppMsg::Archive(ArchiveMsg::PromptArchivePassword {
                    archive_path,
                    prefix,
                    wrong_password: true,
                }));
            }
            Err(ArchiveError::Other(e)) => {
                sender.input(AppMsg::Window(WindowMsg::ShowToast(e)));
            }
            Ok(entries) => {
                if let Some(password) = &password {
                    self.archive
                        .passwords
                        .insert(archive_path.clone(), password.clone());
                    // A password only reaches this point after a prompt or a keyring
                    // hit, so the archive is encrypted and now unlocked.
                    self.archive.known_encrypted.insert(archive_path.clone());
                    self.sync_archive_lock_badges(&archive_path);
                }

                if let Some(confirmed) = self
                    .archive
                    .pending_keyring_saves
                    .confirm(&archive_path, password.as_deref())
                {
                    self.handle_save_archive_password_to_keyring(
                        archive_path.clone(),
                        confirmed,
                        sender,
                    );
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

                // Resolve the virtual-dir flag before sorting, so archives nested
                // inside this archive group with the folders. Runs in the Rayon
                // pool because `is_browsable_archive` can fall back to MIME sniffing.
                // Skipped entirely when the option is off, so nothing changes.
                if archives_as_folders {
                    loader_pool().install(|| {
                        items.par_iter_mut().for_each(|item| {
                            item.is_virtual_dir =
                                !item.is_dir && archive::is_browsable_archive(&item.target_path);
                        });
                    });
                }

                loader_pool().install(|| {
                    items.par_sort_unstable_by(move |a, b| {
                        let group = a.cmp_folder_group(b, folders_first);
                        if group != std::cmp::Ordering::Equal {
                            return group;
                        }

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

                    // Archives nested inside an archive follow the same opt-in rule.
                    let is_virtual_dir = item.is_virtual_dir;

                    let icon = if is_virtual_dir {
                        utils::icon::get_icon_for_virtual_dir(&item.target_path)
                    } else {
                        utils::icon::get_icon_for_path(&item.target_path, item.is_dir)
                    };
                    let original_icon = if is_virtual_dir {
                        Some(utils::icon::get_icon_for_path(&item.target_path, false))
                    } else {
                        None
                    };

                    if !item.is_dir {
                        let (is_img, is_vid) = is_visual_media_by_ext(&item.target_path);
                        let is_audio = is_audio_file(&item.target_path);
                        if is_img || is_vid || is_audio {
                            media_tasks.push((grid_idx, item.target_path.clone()));
                        }
                    }

                    let hidden_exts = crate::utils::helpers::effective_hidden_extensions(
                        &item.display_name,
                        is_virtual_dir,
                        &self.config.ui.hidden_extensions,
                    );
                    let display_label = crate::utils::helpers::format_display_label(
                        &item.display_name,
                        item.is_dir,
                        &hidden_exts,
                    );

                    self.tabs[self.active_tab_index].files.append(
                        FileItem::builder(item.display_name.clone(), item.target_path, icon)
                            .display_label(display_label)
                            .is_dir(item.is_dir)
                            .is_virtual_dir(is_virtual_dir)
                            .original_icon(original_icon)
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

                // Navigate mode: land on the first item so the arrow keys work at once.
                self.select_first_for_navigate();

                if !self.config.ui.lazy_thumbnails {
                    self.spawn_thumbnail_loader(
                        media_tasks,
                        current_session,
                        self.active_tab_index,
                        sender.clone(),
                    );
                } else {
                    sender.input(AppMsg::Thumbnails(ThumbnailsMsg::CheckVisibleThumbnails));
                }
            }
        }
    }
}
