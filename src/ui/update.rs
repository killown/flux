use crate::model::{AppMsg, FluxApp};
use crate::ui::FileProperties;
use crate::utils;
use adw::gio::prelude::*;
use adw::prelude::*;
use relm4::prelude::*;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn reset_from_content_search(&mut self) {
        if let Some(cancellable) = self.content_search_cancellable.take() {
            cancellable.cancel();
        }
        self.is_content_searching = false;
        self.load_id.fetch_add(1, Ordering::SeqCst);
        self.pending_thumbnails.clear();
        self.filter.clear();
        self.search_just_opened = false;
        self.is_list_mode = self.saved_list_mode;
        self.files.view.set_max_columns(self.saved_max_columns);
        self.files.clear();
        self.sync_list_mode();
    }

    pub fn handle_update(&mut self, message: AppMsg, sender: relm4::AsyncComponentSender<Self>) {
        match message {
            // ==========================================
            // Sidebar Operations
            // ==========================================
            AppMsg::RefreshSidebar => self.handle_refresh_sidebar(),
            AppMsg::RemoveFromSidebar(path) => self.handle_remove_from_sidebar(path),
            AppMsg::AddToSidebarPermanent => self.handle_add_to_sidebar_permanent(),
            AppMsg::ReorderSidebar { from, to } => self.handle_reorder_sidebar(from, to),
            AppMsg::PromptSidebarRename { path, current_name } => {
                let path_str = path.to_string_lossy();
                if let Some(section_name) = path_str.strip_prefix("__section__:") {
                    // Rename a section label rather than a pinned path.
                    self.show_prompt_sidebar_rename_section(
                        section_name.to_string(),
                        current_name,
                        &sender,
                    );
                } else {
                    self.show_prompt_sidebar_rename(path, current_name, &sender);
                }
            }
            AppMsg::RenameSidebarPlace { path, new_name } => {
                self.handle_rename_sidebar_place(path, new_name, &sender);
            }
            AppMsg::PromptSidebarRenameSection {
                old_name,
                current_name,
            } => {
                self.show_prompt_sidebar_rename_section(old_name, current_name, &sender);
            }
            AppMsg::RenameSidebarSection { old_name, new_name } => {
                let mut modified = false;
                for place in &mut self.config.sidebar {
                    if place.kind.as_deref() == Some("label") && place.name == old_name {
                        place.name = new_name.clone();
                        modified = true;
                    }
                }
                if modified {
                    crate::utils::save_config(&self.config);
                    self.refresh_sidebar();
                }
            }
            AppMsg::RemoveSidebarSection(name) => self.handle_remove_sidebar_section(name),
            AppMsg::PromptNewSidebarSection => {
                self.show_prompt_new_sidebar_section(&sender);
            }
            AppMsg::AddSidebarSection(title) => self.handle_add_sidebar_section(title),
            AppMsg::SidebarDropMove {
                source_paths,
                dest_path,
            } => self.handle_sidebar_drop_move(source_paths, dest_path, &sender),
            AppMsg::PinFolderAt {
                path,
                before,
                label_name,
            } => self.handle_pin_folder_at(path, before, label_name),
            AppMsg::ShowSidebarPinZone(_val) => {}
            AppMsg::ToggleSidebar => self.handle_toggle_sidebar(),
            AppMsg::SetSidebarWidth(val) => self.handle_set_sidebar_width(val),
            AppMsg::ShowSidebarIconPicker(path) => {
                self.show_sidebar_icon_picker(path, &sender);
            }

            // ==========================================
            // Navigation & History
            // ==========================================
            AppMsg::Navigate(path) => self.handle_navigate(path, &sender),
            AppMsg::GoBack => self.handle_go_back(&sender),
            AppMsg::GoForward => self.handle_go_forward(&sender),
            AppMsg::SyncPathEntry => {}
            AppMsg::PromptLocationDialog => FluxApp::show_location_dialog(self, sender),
            AppMsg::JumpToRecent(rank) => {
                let target_index = if rank == 0 { 0 } else { rank - 1 };
                if let Some(target_path) = self.recent_stack.get(target_index).cloned() {
                    if rank != 0 && target_path == self.current_path {
                        return;
                    }
                    sender.input(AppMsg::Navigate(target_path));
                }
            }
            AppMsg::ClearRecents => self.handle_clear_recents(&sender),
            AppMsg::SetShowRecents(val) => self.handle_set_show_recents(val, &sender),
            AppMsg::SetRecentsRow(val) => self.handle_set_recents_row(val, &sender),
            AppMsg::SetMaxHistory(val) => {
                self.handle_set_max_history(val);
            }

            // ==========================================
            // Directory Loading & Cache
            // ==========================================
            AppMsg::FolderLoadedChunk {
                load_id,
                chunk,
                is_cached,
            } => {
                if self.load_id.load(std::sync::atomic::Ordering::SeqCst) == load_id {
                    self.append_context_batch(chunk, load_id, is_cached, &sender);
                }
            }
            AppMsg::FolderLoadedFinish { load_id } => {
                if self.load_id.load(std::sync::atomic::Ordering::SeqCst) == load_id {
                    self.is_loading = false;
                    unsafe {
                        libc::malloc_trim(0);
                    }
                }
            }
            AppMsg::FolderLoaded {
                path,
                load_id,
                items,
                media_tasks,
            } => {
                self.handle_folder_loaded(path, load_id, items, media_tasks, &sender);
            }
            AppMsg::InvalidateCacheAndNavigate(path) => {
                self.folder_cache.remove(&self.current_path);
                sender.input(AppMsg::Navigate(path));
            }
            AppMsg::SetFolderCacheCapacity(val) => {
                self.handle_set_folder_cache_capacity(val);
            }
            AppMsg::SetLoaderBatchSize(val) => {
                self.handle_set_loader_batch_size(val);
            }

            // ==========================================
            // Archives
            // ==========================================
            AppMsg::EnterArchive(archive_path) => self.handle_enter_archive(archive_path, &sender),
            AppMsg::ArchiveLoaded {
                archive_path,
                prefix,
                password,
                load_id,
                result,
            } => {
                self.handle_archive_loaded(archive_path, prefix, password, load_id, result, &sender)
            }
            AppMsg::PromptArchivePassword {
                archive_path,
                prefix,
                wrong_password,
            } => self.show_prompt_archive_password(archive_path, prefix, wrong_password, &sender),
            AppMsg::LoadArchiveWithPassword {
                archive_path,
                prefix,
                password,
            } => {
                self.archive_locked = false;
                self.load_archive(archive_path, prefix, Some(password), &sender);
                self.update_breadcrumbs();
            }

            // ==========================================
            // View, Grid & Presentation
            // ==========================================
            AppMsg::SelectionChanged => self.handle_selection_changed(&sender),
            AppMsg::ToggleListMode => self.handle_toggle_list_mode(),
            AppMsg::ToggleSortOrder => self.handle_toggle_sort_order(&sender),
            AppMsg::CycleSort => self.handle_cycle_sort(&sender),
            AppMsg::CycleFolderPriority => self.handle_cycle_folder_priority(&sender),
            AppMsg::SetAsc(asc) => self.handle_set_asc(asc, &sender),
            AppMsg::SetDefaultSort(sort) => self.handle_set_default_sort(sort, &sender),
            AppMsg::SetFoldersFirst(val) => self.handle_set_folders_first(val, &sender),
            AppMsg::SetGridSpacing(val) => self.handle_set_grid_spacing(val, &sender),
            AppMsg::SetMaxWidthChars(val) => self.handle_set_max_width_chars(val, &sender),
            AppMsg::SetExpandLabels(val) => self.handle_set_expand_labels(val, &sender),
            AppMsg::Zoom(delta) => self.handle_zoom(delta),
            AppMsg::SwitchHeader(view_name) => self.handle_switch_header(view_name),

            // ==========================================
            // Icons & Metadata
            // ==========================================
            AppMsg::SetIconSize(val) => self.handle_set_icon_size(val, &sender),
            AppMsg::SetListIconSize(val) => self.handle_set_list_icon_size(val, &sender),
            AppMsg::SetShowEmptyDirEmblem(val) => {
                self.config.ui.show_empty_dir_emblem = val;
                utils::save_config(&self.config);
            }
            AppMsg::SetFileIcon { path, image_path } => {
                self.handle_set_file_icon(path, image_path, &sender)
            }
            AppMsg::ResetFileIcon(path) => self.handle_reset_file_icon(path, &sender),
            AppMsg::SetFolderIcon { path, icon_name } => {
                self.handle_set_folder_icon(path, icon_name, &sender)
            }
            AppMsg::ResetFolderIcon(path) => self.handle_reset_folder_icon(path, &sender),
            AppMsg::TriggerResetIcon => {
                let target = self
                    .get_selected_path()
                    .unwrap_or_else(|| self.current_path.clone());
                if target.is_dir() {
                    sender.input(AppMsg::ResetFolderIcon(target.clone()));
                    sender.input(AppMsg::ResetFileIcon(target));
                } else {
                    sender.input(AppMsg::ResetFileIcon(target));
                }
            }
            AppMsg::ShowIconPicker(target_path) => self.show_icon_picker(target_path, &sender),
            AppMsg::TriggerIconPicker => {
                let target = self
                    .get_selected_path()
                    .unwrap_or_else(|| self.current_path.clone());
                if target.is_dir() {
                    self.show_icon_picker(target, &sender);
                }
            }
            AppMsg::FolderIconsReady { icons, session } => {
                if session == self.load_id.load(Ordering::SeqCst) {
                    for i in 0..self.files.len() {
                        if let Some(wrapper) = self.files.get(i) {
                            let path_key = wrapper.borrow().path.to_string_lossy().to_string();
                            if let Some(icon_name) = icons.get(&path_key) {
                                if let Ok(icon) = gtk::gio::Icon::for_string(icon_name) {
                                    let mut item = wrapper.borrow().clone();
                                    item.icon = icon;
                                    item.is_custom_icon = true;
                                    self.files.remove(i);
                                    self.files.insert(i, item);
                                }
                            }
                        }
                    }
                }
            }
            AppMsg::MediaDurationReady(maybe_duration) => {
                self.handle_media_duration_ready(maybe_duration)
            }
            AppMsg::FileMetaReady { mime, dimensions } => {
                self.handle_file_meta_ready(mime, dimensions)
            }

            // ==========================================
            // Thumbnails & FFmpeg
            // ==========================================
            AppMsg::SetShowThumbnails(val) => self.handle_set_show_thumbnails(val, &sender),
            AppMsg::SetLazyThumbnails(val) => {
                self.handle_set_lazy_thumbnails(val);
            }
            AppMsg::SetThumbnailThreads(val) => {
                self.handle_set_thumbnail_threads(val);
            }
            AppMsg::SetThumbnailType { type_name, enabled } => {
                self.handle_set_thumbnail_type(type_name, enabled, &sender)
            }
            AppMsg::CheckVisibleThumbnails => {
                self.check_visible_thumbnails(&sender);
            }
            AppMsg::RequestThumbnail {
                grid_idx,
                path,
                load_id: _,
            } => self.handle_request_thumbnail(grid_idx, path, &sender),
            AppMsg::ThumbnailReady {
                grid_idx,
                texture,
                load_id,
            } => self.handle_thumbnail_ready(grid_idx, texture, load_id),
            AppMsg::SetFfmpegThreads(val) => {
                self.handle_set_ffmpeg_threads(val);
            }
            AppMsg::SetFfmpegSeekSeconds(val) => {
                self.handle_set_ffmpeg_seek_seconds(val);
            }
            AppMsg::SetFfmpegAutoRotate(val) => {
                self.handle_set_ffmpeg_auto_rotate(val);
            }

            // ==========================================
            // Search & Filtering
            // ==========================================
            AppMsg::UpdateFilter(query) => self.handle_update_filter(query, &sender),
            AppMsg::SearchInput(c) => self.handle_search_input(c),
            AppMsg::SearchBackspace => self.handle_search_backspace(&sender),
            AppMsg::CloseSearchSync => {
                self.search_just_opened = false;
            }
            AppMsg::SetExtensionFilter(patterns) => {
                self.handle_set_extension_filter(patterns, &sender);
            }
            AppMsg::ClearExtensionFilter => {
                self.handle_clear_extension_filter(&sender);
            }
            AppMsg::OpenAdvancedSearch => {
                crate::ui::advanced_search::show_advanced_search(self, sender.clone());
            }
            AppMsg::StartAdvancedSearch(params) => {
                crate::services::extension_search::start_advanced_search(self, params, sender);
            }
            AppMsg::StartExtensionSearch(patterns) => {
                crate::services::extension_search::start_extension_search(self, patterns, sender);
            }
            AppMsg::ExtensionSearchBatch { results, session } => {
                self.handle_extension_search_batch(results, session);
            }
            AppMsg::StartContentSearch(term, ext_filter) => {
                crate::services::content_search::start_content_search(
                    self, term, ext_filter, sender,
                )
            }
            AppMsg::ContentSearchResult {
                path,
                line,
                line_number,
                session,
            } => {
                self.handle_content_search_result(path, line, line_number, session);
            }
            AppMsg::ContentSearchDone { session } => self.handle_content_search_done(session),
            AppMsg::CancelContentSearch => {
                sender.input(AppMsg::Refresh);
                self.reset_from_content_search();
            }
            AppMsg::SetMaxSearchResults(val) => {
                self.handle_set_max_search_results(val);
            }
            AppMsg::SetMaxContentSearchResults(val) => {
                self.handle_set_max_content_search_results(val, &sender)
            }

            // ==========================================
            // Tags
            // ==========================================
            AppMsg::OpenTagPicker => {
                let selection = self.get_selection();
                let paths = if selection.is_empty() {
                    vec![self.current_path.clone()]
                } else {
                    selection
                };

                let state_db = self.state_db.clone();
                let sender_tag = sender.clone();
                let paths_clone = paths.clone();

                std::thread::spawn(move || {
                    let first_path = &paths_clone[0];
                    let initial_tags = crate::utils::xattr::read_tags(first_path);
                    let available_tags = state_db.list_all_tags().unwrap_or_default();

                    sender_tag.input(AppMsg::TagsReady {
                        paths: paths_clone,
                        tags: initial_tags,
                        available_tags,
                    });
                });
            }
            AppMsg::TagsReady {
                paths,
                tags,
                available_tags,
            } => {
                let parent_widget = self.files.view.clone();
                crate::ui::tag_picker::show_tag_picker(
                    &parent_widget,
                    paths,
                    tags,
                    available_tags,
                    sender.clone(),
                );
            }
            AppMsg::SetFileTags { path, tags } => {
                let state_db = self.state_db.clone();
                let path_clone = path.clone();
                let tags_clone = tags.clone();
                let sender_refresh = sender.clone();

                std::thread::spawn(move || {
                    let _ = crate::utils::xattr::write_tags(&path_clone, &tags_clone);
                    let mtime = std::fs::metadata(&path_clone)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);

                    let _ = state_db.set_tags(&path_clone, &tags_clone, mtime);
                    sender_refresh.input(AppMsg::Refresh);
                });
            }
            AppMsg::DeleteTagGlobally(tag) => {
                let state_db = self.state_db.clone();
                let sender_refresh = sender.clone();
                std::thread::spawn(move || {
                    let _ = state_db.delete_tag_globally(&tag);
                    sender_refresh.input(AppMsg::Refresh);
                });
            }
            AppMsg::OpenTagNavigator => {
                let tags = self.state_db.list_all_tags().unwrap_or_default();
                crate::ui::tag_navigator::show_tag_navigator(
                    &self.files.view,
                    tags,
                    sender.clone(),
                );
            }
            AppMsg::NavigateTag(tag) => {
                let filter_str = format!(":tag:{}", tag);
                self.search_just_opened = false;
                self.header_view = crate::ui::constants::VIEW_SEARCH.to_string();
                sender.input(AppMsg::UpdateFilter(filter_str));
            }

            // ==========================================
            // File Operations & Clipboard
            // ==========================================
            AppMsg::Copy => self.handle_copy_or_cut(false, &sender),
            AppMsg::Cut => self.handle_copy_or_cut(true, &sender),
            AppMsg::Paste => self.handle_paste_from_clipboard(&sender),
            AppMsg::PasteImageFromClipboard => {
                // No-op: handled inline in clipboard_paste.rs via read_texture_async.
            }
            AppMsg::PasteTextFromClipboard => {
                // No-op: handled inline in clipboard_paste.rs via read_text_async.
            }
            AppMsg::PasteHtmlFromClipboard => {
                // No-op: handled inline in clipboard_paste.rs via read_async.
            }
            AppMsg::ConfirmReplacePaste {
                files,
                conflicts,
                is_cut,
            } => self.show_confirm_replace_paste(files, conflicts, is_cut, &sender),
            AppMsg::PerformPaste { files, is_cut } => {
                self.perform_paste(files, is_cut, sender.clone())
            }
            AppMsg::PerformPasteForced { files, is_cut } => {
                self.perform_paste_inner(files, is_cut, true, sender.clone())
            }
            AppMsg::MoveFilesToTarget {
                sources,
                destination,
            } => {
                self.handle_move_files_to_target(sources, destination, &sender);
            }
            AppMsg::PerformRename(old_path, new_name) => {
                self.handle_perform_rename(old_path, new_name, &sender)
            }
            AppMsg::Delete => {
                let selection = self.get_selection();
                if self.is_content_searching {
                    self.remove_search_results_for_paths(&selection);
                }
                crate::services::trash::delete_items(
                    selection,
                    self.active_item_path.clone(),
                    sender,
                );
            }
            AppMsg::EmptyTrash => self.handle_empty_trash(&sender),
            AppMsg::RestoreItem(_) => {
                sender.input(AppMsg::Refresh);
            }
            AppMsg::PromptNewFolder => self.show_prompt_new_folder(&sender),
            AppMsg::PromptNewFile => self.show_prompt_new_file(&sender),
            AppMsg::OpenFileProperties(path) => {
                let properties_win = FileProperties::builder().launch(path).detach();
                properties_win.widget().present();
            }

            // ==========================================
            // Drag and Drop
            // ==========================================
            AppMsg::SetDisableDragAndDrop(val) => {
                self.handle_set_disable_drag_and_drop(val);
            }
            AppMsg::HandleDrop {
                source_paths,
                dest_path,
            } => self.handle_drop_items(source_paths, dest_path, &sender),
            AppMsg::HandleExternalDrop {
                source_paths,
                dest_path,
            } => self.handle_external_drop_items(source_paths, dest_path, &sender),

            // ==========================================
            // Undo & Redo System
            // ==========================================
            AppMsg::Undo => self.handle_undo(&sender),
            AppMsg::Redo => self.handle_redo(&sender),
            AppMsg::TrashSucceeded(paths) => {
                self.file_op_history
                    .push_undo(crate::ui::undo_redo::FileOp::Trash { paths });
            }
            AppMsg::MoveSucceeded { items, dest_dir } => {
                self.file_op_history
                    .push_undo(crate::ui::undo_redo::FileOp::Move { items, dest_dir });
            }
            AppMsg::CopySucceeded { copies, dest_dir } => {
                let copy_dests = copies.into_iter().map(|(_, dest)| dest).collect();
                self.folder_cache.remove(&dest_dir);
                self.file_op_history
                    .push_undo(crate::ui::undo_redo::FileOp::Copy {
                        copies: copy_dests,
                        dest_dir,
                    });
            }
            AppMsg::UndoMoveComplete {
                redo_items,
                dest_dir,
            } => {
                self.handle_undo_move_complete(redo_items, dest_dir, &sender);
            }
            AppMsg::UndoMoveFailed(op) => {
                self.handle_undo_move_failed(op);
            }
            AppMsg::UndoTrashComplete { paths } => {
                self.handle_undo_trash_complete(paths, &sender);
            }
            AppMsg::UndoTrashFailed(op) => {
                self.handle_undo_trash_failed(op);
            }
            AppMsg::RedoMoveComplete { items, dest_dir } => {
                self.handle_redo_move_complete(items, dest_dir, &sender);
            }
            AppMsg::RedoMoveFailed(op) => {
                self.handle_redo_move_failed(op);
            }
            AppMsg::RedoTrashComplete { paths } => {
                self.handle_redo_trash_complete(paths, &sender);
            }
            AppMsg::RedoTrashFailed(op) => {
                self.handle_redo_trash_failed(op);
            }

            // ==========================================
            // File Conflicts & Dialogs
            // ==========================================
            AppMsg::FileConflictDetected { context, resolver } => {
                if let Some(mut dialog) = self.transfer_dialog.take() {
                    dialog.close();
                }

                let tx = resolver
                    .lock()
                    .expect("conflict resolver mutex poisoned")
                    .take()
                    .expect("FileConflictDetected handled more than once");

                self.conflict_dialog_active = true;
                crate::ui::conflict_dialog::show_conflict_dialog(context, tx, sender.clone());
            }
            AppMsg::ConflictDialogClosed => {
                self.conflict_dialog_active = false;
            }
            AppMsg::SetConflictPolicy(_policy) => {
                // No-op for now, extend when a persistent default preference
                // is added to Settings.
            }

            // ==========================================
            // Quick Panel / Exclusive List
            // ==========================================
            AppMsg::AddExclusive(explicit_path) => {
                self.handle_add_exclusive(explicit_path, &sender)
            }
            AppMsg::ClearExclusive => self.handle_clear_exclusive(&sender),
            AppMsg::RemoveQuickItem(path) => self.handle_remove_quick_item(path, &sender),
            AppMsg::RebuildQuickPanel => self.handle_rebuild_quick_panel(&sender),
            AppMsg::NextExclusive => self.handle_next_exclusive(&sender),
            AppMsg::PrevExclusive => self.handle_prev_exclusive(&sender),

            // ==========================================
            // File Watcher Notifications
            // ==========================================
            AppMsg::FileDeleted(path) => self.handle_file_deleted(path),
            AppMsg::FileChanged(path) => self.handle_file_changed(path, &sender),
            AppMsg::StartRename(path) => self.handle_start_rename(path),
            AppMsg::TriggerRenameSelection => self.handle_trigger_rename_selection(&sender),
            AppMsg::ItemMoved { old_path, new_path } => {
                let old_key = old_path.to_string_lossy().to_string();
                let new_key = new_path.to_string_lossy().to_string();
                if let Some(v) = self.config.ui.file_icons.remove(&old_key) {
                    self.config.ui.file_icons.insert(new_key.clone(), v);
                }
                if let Some(v) = self.config.ui.folder_icons.remove(&old_key) {
                    self.config.ui.folder_icons.insert(new_key, v);
                }
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }

            // ==========================================
            // Task Queue & Background Transfers
            // ==========================================
            AppMsg::TaskProgress {
                id,
                label,
                current,
                total,
                total_items,
                cancellable,
            } => self.handle_task_progress(id, label, current, total, total_items, cancellable),
            AppMsg::TaskCompleted(id) => self.handle_task_completed(id),
            AppMsg::CancelTask(id) => self.handle_cancel_task(id, &sender),
            AppMsg::CancelAllTasks => self.handle_cancel_all_tasks(&sender),
            AppMsg::TaskQueueTick => self.handle_task_queue_tick(&sender),
            AppMsg::ShowTransferDialog => self.handle_show_transfer_dialog(),
            AppMsg::ShowTransferDialogIfActive(id) => {
                self.handle_show_transfer_dialog_if_active(id)
            }
            AppMsg::TransferDialogClosed => self.handle_transfer_dialog_closed(),

            // ==========================================
            // Commands & External Apps
            // ==========================================
            AppMsg::Open(position) => self.handle_open(position, &sender),
            AppMsg::Activate => self.handle_activate(&sender),
            AppMsg::LaunchWithApp(app_id) => self.handle_launch_with_app(app_id),
            AppMsg::ExecuteCommand(cmd_template) => {
                self.handle_execute_command(cmd_template, &sender)
            }
            AppMsg::ToggleNoCommandDialog(action_name) => {
                if let Some(action) = self
                    .menu_actions
                    .iter_mut()
                    .find(|a| a.action_name == action_name)
                {
                    action.no_command_dialog = !action.no_command_dialog;
                    utils::save_config(&self.config);
                    if let Err(e) = utils::save_menu_config(&self.menu_actions) {
                        eprintln!("Failed to save menu.rs: {}", e);
                    }
                }
            }
            AppMsg::RefreshCommandDialog(action_name) => {
                if let Some(dialog) = &self.command_dialog {
                    if let Some(action) = self
                        .menu_actions
                        .iter()
                        .find(|a| a.action_name == action_name)
                    {
                        dialog.update_switch_state(action.no_command_dialog);
                    }
                }
            }
            AppMsg::ShowCommandDialog(id) => {
                self.handle_show_command_dialog(id);
            }
            AppMsg::ShowCommandDialogIfActive(id) => {
                self.handle_show_command_dialog_if_active(id);
            }
            AppMsg::CommandOutput {
                id,
                line,
                is_stderr,
            } => {
                self.task_queue.append_output(id, line.clone());
                let prefix = if is_stderr { "stderr" } else { "stdout" };
                eprintln!("[task {}] {}: {}", id, prefix, line);

                if let Some(dialog) = &self.command_dialog {
                    if dialog.task_id == id {
                        dialog.append_output(&line);
                    }
                }
            }
            AppMsg::CommandDialogClosed => {
                self.handle_command_dialog_closed();
            }
            AppMsg::CommandFinished {
                id,
                success,
                exit_code,
            } => {
                if !success {
                    let msg = if let Some(code) = exit_code {
                        format!("Command failed with exit code {}", code)
                    } else {
                        "Command was terminated".to_string()
                    };
                    sender.input(AppMsg::ShowToast(msg));
                }

                if let Some(dialog) = &self.command_dialog {
                    if dialog.task_id == id {
                        self.handle_command_dialog_closed();
                    }
                }

                sender.input(AppMsg::TaskCompleted(id));
            }

            // ==========================================
            // Network & Remote Operations
            // ==========================================
            AppMsg::NetworkLoaded { uri, contexts } => self.handle_network_loaded(uri, contexts),
            AppMsg::ConnectToServer { uri, credentials } => {
                self.handle_connect_to_server(uri, credentials, &sender)
            }
            AppMsg::UnmountNetwork(uri) => self.handle_unmount_network(uri, &sender),
            AppMsg::AddNetworkBookmark { name, uri } => {
                self.handle_add_network_bookmark(name, uri, &sender)
            }
            AppMsg::RemoveNetworkBookmark(uri) => self.handle_remove_network_bookmark(uri, &sender),
            AppMsg::RefreshNetworkSidebar => self.handle_refresh_network_sidebar(&sender),
            AppMsg::NavigateNetwork => self.handle_navigate_network(&sender),
            AppMsg::PromptNetworkCredentials {
                uri,
                message,
                flags,
                auth_failed,
            } => {
                let window = gtk::Application::default().active_window().unwrap();
                crate::ui::network_dialogs::show_credentials_dialog(
                    &window,
                    uri,
                    message,
                    flags,
                    auth_failed,
                    sender.input_sender().clone(),
                );
            }

            // ==========================================
            // Mounts & Disks
            // ==========================================
            AppMsg::SystemMountsReady(mounts) => {
                self.handle_system_mounts_ready(mounts);
            }
            AppMsg::UnmountDevice(path) => self.handle_unmount_device(path, &sender),
            AppMsg::UnlockLuksImage { path } => {
                self.show_luks_passphrase_dialog(path, &sender);
            }
            AppMsg::LuksMounted {
                image_path: _,
                mount_point,
            } => {
                sender.input(AppMsg::Navigate(mount_point));
                sender.input(AppMsg::ShowToast(crate::i18n::tr("Volume mounted.")));
            }

            // ==========================================
            // Embedded Terminal
            // ==========================================
            AppMsg::ToggleTerminal => self.handle_toggle_terminal(),
            AppMsg::SetTerminalHeight(h) => {
                self.handle_set_terminal_config(Some(h), None, None, None)
            }
            AppMsg::SetTerminalFont(f) => {
                self.handle_set_terminal_config(None, Some(f), None, None)
            }
            AppMsg::SetTerminalFgColor(c) => {
                self.handle_set_terminal_config(None, None, Some(c), None)
            }
            AppMsg::SetTerminalBgColor(c) => {
                self.handle_set_terminal_config(None, None, None, Some(c))
            }

            // ==========================================
            // Context Menus & Popups
            // ==========================================
            AppMsg::PrepareContextMenu(x, y, item_idx) => {
                self.handle_prepare_context_menu(x, y, item_idx, &sender);
            }
            AppMsg::PrepareSecondaryMenu { x, y, path } => {
                self.handle_prepare_secondary_menu(x, y, path, &sender);
            }
            AppMsg::ShowContextMenu { x, y, path, mime } => {
                self.build_and_show_context_menu(x, y, path, mime, &sender)
            }
            AppMsg::ShowSecondaryMenu {
                x,
                y,
                path,
                mime,
                actions,
            } => {
                self.build_and_show_secondary_menu(x, y, path, mime, actions, &sender);
            }

            // ==========================================
            // Window, Shell & General Preferences
            // ==========================================
            AppMsg::Refresh => self.handle_refresh_path(&sender),
            AppMsg::SetSingleClick(val) => self.handle_set_single_click(val),
            AppMsg::ToggleSingleClick => self.handle_toggle_single_click(),
            AppMsg::SetShowHidden(val) => self.handle_set_show_hidden(val, &sender),
            AppMsg::ToggleHidden => self.handle_set_show_hidden(!self.show_hidden, &sender),
            AppMsg::SetShowCsd(val) => self.handle_set_show_csd(val),
            AppMsg::SetWindowControlsLeft(val) => self.handle_set_window_controls_left(val),
            AppMsg::SetShowXdgDirs(val) => self.handle_set_show_xdg_dirs(val, &sender),
            AppMsg::SetTheme(theme) => self.handle_set_theme(theme),
            AppMsg::SetShortcut(key, val) => self.handle_set_shortcut(key, val),
            AppMsg::SetMaximized(max) => self.handle_set_maximized(max),
            AppMsg::SetWindowWidth(val) => self.handle_set_window_size(Some(val), None),
            AppMsg::SetWindowHeight(val) => self.handle_set_window_size(None, Some(val)),
            AppMsg::ShowAbout => FluxApp::show_about_window(),
            AppMsg::ShowHelp => {
                let help_win = crate::ui::HelpWindow::builder().launch(()).detach();
                help_win.widget().present();
            }
            AppMsg::OpenDebugWindow => {
                crate::ui::debug::show_debug_window(self);
            }
            AppMsg::ShowToast(msg) => {
                if let Some(prev) = self.last_toast.take() {
                    prev.dismiss();
                }
                let toast = adw::Toast::new(&msg);
                self.toast_overlay.add_toast(toast.clone());
                self.last_toast = Some(toast);
            }
        }
    }
}
