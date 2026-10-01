use crate::model::{AppMsg, FluxApp, RightPanelType};
use crate::ui::FileProperties;
use adw::prelude::*;
use relm4::prelude::*;

impl FluxApp {
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
                self.handle_prompt_sidebar_rename(path, current_name, &sender)
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
                self.handle_rename_sidebar_section(old_name, new_name)
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
            AppMsg::Navigate(path) => {
                self.stop_video_preview();
                self.handle_navigate(path, &sender);
            }
            AppMsg::GoBack => {
                self.stop_video_preview();
                self.handle_go_back(&sender);
            }
            AppMsg::GoForward => {
                self.stop_video_preview();
                self.handle_go_forward(&sender);
                sender.input(AppMsg::Refresh);
            }
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
            AppMsg::ShowLoadingSpinner(session) => self.handle_show_loading_spinner(session),
            AppMsg::FolderLoadedChunk {
                load_id,
                chunk,
                is_cached,
            } => self.handle_folder_loaded_chunk(load_id, chunk, is_cached, &sender),
            AppMsg::FolderLoadedFinish { load_id } => self.handle_folder_loaded_finish(load_id),
            AppMsg::FolderLoaded {
                path,
                load_id,
                items,
                media_tasks,
            } => self.handle_folder_loaded_begin(path, load_id, items, media_tasks, &sender),
            AppMsg::InvalidateCacheAndNavigate(path) => {
                self.handle_invalidate_cache_and_navigate(path, &sender)
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
            AppMsg::PromptArchiveDeletion {
                archive_path,
                inner_path,
            } => {
                self.show_archive_deletion_warning(archive_path, inner_path, &sender);
            }
            AppMsg::ExtractArchive => {
                self.handle_extract_archive(&sender);
            }
            AppMsg::EnterArchive(archive_path) => {
                self.stop_video_preview();
                self.handle_enter_archive(archive_path, &sender)
            }
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
            AppMsg::SetAutoMimeBodyColor(color) => {
                self.handle_set_auto_mime_body_color(color, &sender)
            }
            AppMsg::SetAutoMimeFontColor(color) => {
                self.handle_set_auto_mime_font_color(color, &sender)
            }
            AppMsg::SetAutoGenerateMimeIcons(enabled) => {
                self.handle_set_auto_generate_mime_icons(enabled, &sender)
            }
            AppMsg::SetAutoMimeAccentColor(color) => {
                self.handle_set_auto_mime_accent_color(color, &sender)
            }
            AppMsg::SetAutoMimeFontSize(size) => self.handle_set_auto_mime_font_size(size, &sender),
            AppMsg::ResetExtensionIcon(ext) => self.handle_reset_extension_icon(ext, &sender),
            AppMsg::SetIconSize(val) => self.handle_set_icon_size(val, &sender),
            AppMsg::SetListIconSize(val) => self.handle_set_list_icon_size(val, &sender),
            AppMsg::SetShowEmptyDirEmblem(val) => self.handle_set_show_empty_dir_emblem(val),
            AppMsg::SetFileIcon { path, image_path } => {
                self.handle_set_file_icon(path, image_path, &sender)
            }
            AppMsg::ResetFileIcon(path) => self.handle_reset_file_icon(path, &sender),
            AppMsg::SetFolderIcon { path, icon_name } => {
                self.handle_set_folder_icon(path, icon_name, &sender)
            }
            AppMsg::ResetFolderIcon(path) => self.handle_reset_folder_icon(path, &sender),
            AppMsg::TriggerResetIcon => self.handle_trigger_reset_icon(&sender),
            AppMsg::ShowIconPicker(target_path) => self.show_icon_picker(target_path, &sender),
            AppMsg::TriggerIconPicker => self.handle_trigger_icon_picker(&sender),
            AppMsg::FolderIconsReady { icons, session } => {
                self.handle_folder_icons_ready(icons, session)
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
            AppMsg::UpdateVisibleThumbnailsViewport {
                progress_top,
                progress_bottom,
            } => self.handle_update_visible_thumbnails_viewport(
                progress_top,
                progress_bottom,
                &sender,
            ),
            AppMsg::TriggerVideoPreview(path) => {
                self.handle_trigger_video_preview(path);
            }
            AppMsg::SetAutoplayVideoPreviews(val) => {
                self.handle_set_autoplay_video_previews(val);
            }
            AppMsg::SetThumbnailSize(val) => {
                self.handle_set_thumbnail_size(val, &sender);
            }
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
                tab_index,
            } => self.handle_thumbnail_ready(grid_idx, texture, load_id, tab_index),
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
            AppMsg::SetTagPanelWidth(val) => self.handle_set_tag_panel_width(val),
            AppMsg::SetSearchPanelWidth(val) => self.handle_set_search_panel_width(val),
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
            AppMsg::StartAdvancedSearch(params) => {
                self.header_view = crate::ui::constants::VIEW_SEARCH.to_string();
                self.last_search_was_advanced = true;
                crate::services::extension_search::start_advanced_search(self, params, sender);
            }
            AppMsg::StartExtensionSearch(patterns) => {
                crate::services::extension_search::start_extension_search(self, patterns, sender);
            }
            AppMsg::ExtensionSearchBatch { results, session } => {
                self.handle_extension_search_batch(results, session);
            }
            AppMsg::StartContentSearch(term, ext_filter) => {
                self.header_view = crate::ui::constants::VIEW_SEARCH.to_string();
                crate::services::content_search::start_content_search(
                    self, term, ext_filter, sender,
                )
            }
            AppMsg::SetEnableFileIndexing(enabled) => {
                self.config.ui.enable_file_indexing = enabled;
                crate::utils::save_config(&self.config);
                if enabled {
                    crate::services::indexer::build_home_index_async();
                }
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
            AppMsg::ApplyTagsToSelection(tags) => {
                self.handle_apply_tags_to_selection(tags, &sender)
            }
            AppMsg::RemoveTagFromSelection(tag) => {
                self.handle_remove_tag_from_selection(tag, &sender)
            }
            AppMsg::AddTagToSidebar(tag) => {
                self.handle_add_tag_to_sidebar(tag);
            }
            AppMsg::OpenTagPicker => self.handle_open_tag_picker(&sender),
            AppMsg::TagsReady {
                paths,
                tags,
                available_tags,
            } => self.handle_tags_ready(paths, tags, available_tags, &sender),
            AppMsg::SetFileTags { path, tags } => self.handle_set_file_tags(path, tags, &sender),
            AppMsg::DeleteTagGlobally(tag) => self.handle_delete_tag_globally(tag, &sender),
            AppMsg::NavigateTag(tag) => self.handle_navigate_tag(tag, &sender),
            // ==========================================
            // File Operations & Clipboard
            // ==========================================
            AppMsg::CopyPath => self.handle_copy_path(&sender),
            AppMsg::CreateSymlink => self.handle_create_link(false, &sender),
            AppMsg::CreateHardlink => self.handle_create_link(true, &sender),
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
            AppMsg::EmptyTrash => {
                self.folder_cache.remove(
                    &self.cache_key(&std::path::PathBuf::from(crate::ui::constants::TRASH_URI)),
                );
                self.handle_empty_trash(&sender);
            }
            AppMsg::RestoreItem(path) => {
                if let Some(parent) = path.parent() {
                    self.folder_cache.remove(&self.cache_key(parent));
                }
                self.folder_cache
                    .remove(&self.cache_key(&self.current_path));
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
                for p in &paths {
                    if let Some(parent) = p.parent() {
                        self.folder_cache.remove(&self.cache_key(parent));
                    }
                }
                self.folder_cache
                    .remove(&self.cache_key(&self.current_path));
                self.file_op_history
                    .push_undo(crate::ui::undo_redo::FileOp::Trash { paths });
            }
            AppMsg::MoveSucceeded { items, dest_dir } => {
                self.folder_cache.remove(&self.cache_key(&dest_dir));
                self.file_op_history
                    .push_undo(crate::ui::undo_redo::FileOp::Move { items, dest_dir });
            }
            AppMsg::CopySucceeded { copies, dest_dir } => {
                let copy_dests = copies.into_iter().map(|(_, dest)| dest).collect();
                self.folder_cache.remove(&self.cache_key(&dest_dir));
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
                self.folder_cache.remove(&self.cache_key(&dest_dir));
                self.handle_undo_move_complete(redo_items, dest_dir, &sender);
            }
            AppMsg::UndoMoveFailed(op) => {
                self.handle_undo_move_failed(op);
            }
            AppMsg::UndoTrashComplete { paths } => {
                for p in &paths {
                    if let Some(parent) = p.parent() {
                        self.folder_cache.remove(&self.cache_key(parent));
                    }
                }
                self.folder_cache
                    .remove(&self.cache_key(&self.current_path));
                self.handle_undo_trash_complete(paths, &sender);
            }
            AppMsg::UndoTrashFailed(op) => {
                self.handle_undo_trash_failed(op);
            }
            AppMsg::RedoMoveComplete { items, dest_dir } => {
                self.folder_cache.remove(&self.cache_key(&dest_dir));
                self.folder_cache
                    .remove(&self.cache_key(&self.current_path));
                self.handle_redo_move_complete(items, dest_dir, &sender);
            }
            AppMsg::RedoMoveFailed(op) => {
                self.handle_redo_move_failed(op);
            }
            AppMsg::RedoTrashComplete { paths } => {
                for p in &paths {
                    if let Some(parent) = p.parent() {
                        self.folder_cache.remove(&self.cache_key(parent));
                    }
                }
                self.folder_cache
                    .remove(&self.cache_key(&self.current_path));
                self.handle_redo_trash_complete(paths, &sender);
            }
            AppMsg::RedoTrashFailed(op) => {
                self.handle_redo_trash_failed(op);
            }

            // ==========================================
            // File Conflicts & Dialogs
            // ==========================================
            AppMsg::ShowOpenWithDialog(path) => {
                self.show_open_with_dialog(path, &sender);
            }
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
            AppMsg::InspectDirectory(path) => {
                self.show_dir_inspector_dialog(path, &sender);
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
            AppMsg::FileDeleted(path) => self.handle_file_deleted_dispatch(path),
            AppMsg::FileChanged(path) => self.handle_file_changed_dispatch(path, &sender),
            AppMsg::StartRename(path) => self.handle_start_rename(path),
            AppMsg::TriggerRenameSelection => self.handle_trigger_rename_selection(&sender),
            AppMsg::ItemMoved { old_path, new_path } => {
                self.handle_item_moved(old_path, new_path, &sender)
            }

            // ==========================================
            // Task Queue & Background Transfers
            // ==========================================
            AppMsg::PerformQuickTransfer { dest, is_cut } => {
                self.handle_perform_quick_transfer(dest, is_cut, &sender)
            }
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
                self.handle_toggle_no_command_dialog(action_name)
            }
            AppMsg::RefreshCommandDialog(action_name) => {
                self.handle_refresh_command_dialog(action_name)
            }
            AppMsg::ShowCommandDialog(id) => self.handle_show_command_dialog(id),
            AppMsg::ShowCommandDialogIfActive(id) => self.handle_show_command_dialog_if_active(id),
            AppMsg::CommandOutput {
                id,
                line,
                is_stderr,
            } => self.handle_command_output(id, line, is_stderr),
            AppMsg::CommandDialogClosed => self.handle_command_dialog_closed(),
            AppMsg::CommandFinished {
                id,
                success,
                exit_code,
            } => self.handle_command_finished(id, success, exit_code, &sender),

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
            AppMsg::TerminalCwdChanged(path) => {
                self.handle_terminal_cwd_changed(path, &sender);
            }
            AppMsg::SetTerminalShell(shell) => {
                self.config.ui.terminal.shell = shell;
                crate::utils::save_config(&self.config);
            }
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
            AppMsg::PrepareContextMenu(x, y, path) => {
                self.handle_prepare_context_menu(x, y, path, &sender);
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
            AppMsg::NewTab(target_path) => self.handle_new_tab(target_path, &sender),
            AppMsg::OpenTabs(targets) => self.handle_open_tabs(targets, &sender),
            AppMsg::SwitchTab(index) => self.handle_switch_tab(index, &sender),
            AppMsg::CloseTab(index_opt) => self.handle_close_tab(index_opt, &sender),
            AppMsg::NextTab => self.handle_next_tab(),
            AppMsg::PrevTab => self.handle_prev_tab(),

            // ==========================================
            AppMsg::SetBackgroundAlpha { slot, alpha } => {
                self.handle_set_background_alpha(slot, alpha);
            }
            AppMsg::SetFluxBackground { target, slot } => {
                self.handle_set_flux_background(target, slot)
            }
            AppMsg::ClearFluxBackgrounds => self.handle_clear_flux_backgrounds(&sender),
            AppMsg::SetScaleFontWithIcons(val) => {
                self.handle_set_scale_font_with_icons(val, &sender);
            }
            AppMsg::ToggleTagPanel => {
                self.toggle_sidebar_right_panel(RightPanelType::Tag, &sender);
            }
            AppMsg::ToggleSearchPanel => {
                self.toggle_sidebar_right_panel(RightPanelType::Search, &sender);
            }
            AppMsg::SetShowSymlinkEmblem(val) => {
                self.handle_set_show_symlink_emblem(val, &sender);
            }
            AppMsg::SetScrolledToBottom(at_bottom) => {
                self.scrolled_to_bottom = at_bottom;
            }
            AppMsg::ToggleHeaderBar => {
                self.handle_toggle_header_bar();
            }
            AppMsg::ToggleCurrentFoldersFirst => {
                self.handle_toggle_current_folders_first(&sender);
            }
            AppMsg::Refresh => {
                self.active_video_preview = None;
                self.folder_cache.clear();
                self.handle_refresh_path(&sender);
            }
            AppMsg::UpdateExclusiveSlot(index) => {
                if index < self.exclusive_list.len() {
                    self.exclusive_list[index] = self.current_path.clone();
                    self.exclusive_index = Some(index);
                    self.handle_rebuild_quick_panel(&sender);
                    sender.input(AppMsg::ShowToast(crate::i18n::tr(
                        "Quick list slot updated",
                    )));
                }
            }
            AppMsg::SetSingleClick(val) => self.handle_set_single_click(val),
            AppMsg::ToggleSingleClick => self.handle_toggle_single_click(),
            AppMsg::SetShowHidden(val) => self.handle_set_show_hidden(val, &sender),
            AppMsg::ToggleStatusBar => {
                self.statusbar_visible = !self.statusbar_visible;
            }
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
            AppMsg::SetHiddenExtensions(exts) => {
                self.handle_set_hidden_extensions(exts, &sender);
            }
            AppMsg::OpenDebugWindow => {
                crate::ui::debug::show_debug_window(self);
            }
            AppMsg::ShowGitStatusView => {
                self.handle_show_git_status_view(sender);
            }
            AppMsg::SetGitRepoActive(is_active) => {
                self.is_in_git_repo = is_active;
            }
            AppMsg::GitStatusReady {
                path,
                load_id,
                updates,
            } => {
                self.handle_git_status_ready(path, load_id, updates);
            }
            AppMsg::ShowToast(msg) => self.handle_show_toast(msg),
            AppMsg::SetUiScale(scale) => self.handle_set_ui_scale(scale),
        }
    }
}
