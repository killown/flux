use crate::model::{AppMsg, FluxApp};
use crate::ui::FileProperties;
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
        self.filter.clear();
        self.search_just_opened = false;
        self.is_list_mode = self.saved_list_mode;
        self.files.view.set_max_columns(self.saved_max_columns);
        self.files.clear();
        self.sync_list_mode();
    }

    pub fn handle_update(&mut self, message: AppMsg, sender: relm4::AsyncComponentSender<Self>) {
        match message {
            // Sidebar
            AppMsg::RefreshSidebar => self.handle_refresh_sidebar(),
            AppMsg::RemoveFromSidebar(path) => self.handle_remove_from_sidebar(path),
            AppMsg::AddToSidebarPermanent => self.handle_add_to_sidebar_permanent(),
            AppMsg::ReorderSidebar { from, to } => self.handle_reorder_sidebar(from, to),
            AppMsg::PinFolderAt { path, before } => self.handle_pin_folder_at(path, before),
            AppMsg::UnmountDevice(path) => self.handle_unmount_device(path, &sender),
            AppMsg::ToggleSidebar => self.handle_toggle_sidebar(),

            // Configuration & UI
            AppMsg::SetSingleClick(val) => self.handle_set_single_click(val),
            AppMsg::ToggleSingleClick => self.handle_toggle_single_click(),
            AppMsg::SetShowHidden(val) => self.handle_set_show_hidden(val, &sender),
            AppMsg::ToggleHidden => self.handle_set_show_hidden(!self.show_hidden, &sender),
            AppMsg::SetGridSpacing(val) => self.handle_set_grid_spacing(val, &sender),
            AppMsg::SetMaxWidthChars(val) => self.handle_set_max_width_chars(val, &sender),
            AppMsg::SetExpandLabels(val) => self.handle_set_expand_labels(val, &sender),
            AppMsg::SetFoldersFirst(val) => self.handle_set_folders_first(val, &sender),
            AppMsg::SetIconSize(val) => self.handle_set_icon_size(val, &sender),
            AppMsg::SetSidebarWidth(val) => self.handle_set_sidebar_width(val),
            AppMsg::SetShowCsd(val) => self.handle_set_show_csd(val),
            AppMsg::SetShowXdgDirs(val) => self.handle_set_show_xdg_dirs(val, &sender),
            AppMsg::SetTheme(theme) => self.handle_set_theme(theme),
            AppMsg::SetDefaultSort(sort) => self.handle_set_default_sort(sort, &sender),
            AppMsg::SetShortcut(key, val) => self.handle_set_shortcut(key, val),
            AppMsg::SetMaximized(max) => self.handle_set_maximized(max),
            AppMsg::SetWindowWidth(val) => self.handle_set_window_size(Some(val), None),
            AppMsg::SetWindowHeight(val) => self.handle_set_window_size(None, Some(val)),
            AppMsg::SetFolderIcon { path, icon_name } => {
                self.handle_set_folder_icon(path, icon_name, &sender)
            }
            AppMsg::ResetFolderIcon(path) => self.handle_reset_folder_icon(path, &sender),
            AppMsg::SetShowThumbnails(val) => self.handle_set_show_thumbnails(val, &sender),
            AppMsg::SetThumbnailType { type_name, enabled } => {
                self.handle_set_thumbnail_type(type_name, enabled, &sender)
            }
            AppMsg::SetShowRecents(val) => self.handle_set_show_recents(val, &sender),
            AppMsg::SetRecentsRow(val) => self.handle_set_recents_row(val, &sender),
            AppMsg::SetAsc(asc) => self.handle_set_asc(asc, &sender),

            // Clipboard & File Operations
            AppMsg::ConfirmReplacePaste {
                files,
                conflicts,
                is_cut,
            } => self.show_confirm_replace_paste(files, conflicts, is_cut, &sender),
            AppMsg::PerformPasteForced { files, is_cut } => {
                self.perform_paste_inner(files, is_cut, true, sender.clone())
            }
            AppMsg::PerformPaste { files, is_cut } => {
                self.perform_paste(files, is_cut, sender.clone())
            }
            AppMsg::Copy => self.handle_copy_or_cut(false),
            AppMsg::Cut => self.handle_copy_or_cut(true),
            AppMsg::Paste => self.handle_paste_from_clipboard(&sender),
            AppMsg::PerformRename(old_path, new_name) => {
                self.handle_perform_rename(old_path, new_name, &sender)
            }
            AppMsg::ExecuteCommand(cmd_template) => {
                self.handle_execute_command(cmd_template, &sender)
            }
            AppMsg::HandleDrop {
                source_paths,
                dest_path,
            } => self.handle_drop_items(source_paths, dest_path, &sender),
            AppMsg::HandleExternalDrop {
                source_paths,
                dest_path,
            } => self.handle_external_drop_items(source_paths, dest_path, &sender),
            AppMsg::EmptyTrash => self.handle_empty_trash(&sender),
            AppMsg::Delete => {
                let selection = self.get_selection();
                crate::services::trash::delete_items(
                    selection,
                    self.active_item_path.clone(),
                    sender,
                );
            }

            // Navigation & Quick List
            AppMsg::Navigate(path) => self.handle_navigate(path, &sender),
            AppMsg::GoBack => self.handle_go_back(&sender),
            AppMsg::GoForward => self.handle_go_forward(&sender),
            AppMsg::EnterArchive(archive_path) => self.handle_enter_archive(archive_path, &sender),
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
            AppMsg::JumpToRecent(rank) => {
                let target_index = if rank == 0 { 0 } else { rank - 1 };
                if let Some(target_path) = self.recent_stack.get(target_index).cloned() {
                    if rank != 0 && target_path == self.current_path {
                        return;
                    }
                    sender.input(AppMsg::Navigate(target_path));
                }
            }
            AppMsg::AddExclusive(explicit_path) => {
                self.handle_add_exclusive(explicit_path, &sender)
            }
            AppMsg::ClearExclusive => self.handle_clear_exclusive(&sender),
            AppMsg::RemoveQuickItem(path) => self.handle_remove_quick_item(path, &sender),
            AppMsg::RebuildQuickPanel => self.handle_rebuild_quick_panel(&sender),
            AppMsg::NextExclusive => self.handle_next_exclusive(&sender),
            AppMsg::PrevExclusive => self.handle_prev_exclusive(&sender),

            // View & Grid Controls
            AppMsg::SelectionChanged => self.handle_selection_changed(&sender),
            AppMsg::ToggleListMode => self.handle_toggle_list_mode(),
            AppMsg::ToggleSortOrder => self.handle_toggle_sort_order(&sender),
            AppMsg::CycleSort => self.handle_cycle_sort(&sender),
            AppMsg::CycleFolderPriority => self.handle_cycle_folder_priority(&sender),
            AppMsg::Zoom(delta) => self.handle_zoom(delta),
            AppMsg::ThumbnailReady {
                name,
                texture,
                load_id,
            } => self.handle_thumbnail_ready(name, texture, load_id),
            AppMsg::MediaDurationReady(maybe_duration) => {
                self.handle_media_duration_ready(maybe_duration)
            }
            AppMsg::FileMetaReady { mime, dimensions } => {
                self.handle_file_meta_ready(mime, dimensions)
            }

            // Search & Filtering
            AppMsg::CloseSearchSync => {
                self.search_just_opened = false;
            }
            AppMsg::UpdateFilter(query) => self.handle_update_filter(query, &sender),
            AppMsg::SetExtensionFilter(patterns) => {
                let expanded: Vec<String> = patterns
                    .iter()
                    .flat_map(|p| crate::utils::glob::expand_mime_category(p))
                    .collect();
                self.extension_filter = if expanded.is_empty() {
                    None
                } else {
                    Some(expanded)
                };
                sender.input(AppMsg::Refresh);
            }
            AppMsg::ClearExtensionFilter => {
                self.extension_filter = None;
                sender.input(AppMsg::Refresh);
            }
            AppMsg::StartContentSearch(term, ext_filter) => {
                crate::services::content_search::start_content_search(
                    self, term, ext_filter, sender,
                )
            }
            AppMsg::CancelContentSearch => {
                sender.input(AppMsg::Refresh);
                self.reset_from_content_search();
            }
            AppMsg::ContentSearchResult {
                path,
                line,
                line_number,
                session,
            } => self.handle_content_search_result(path, line, line_number, session),
            AppMsg::ContentSearchDone { session } => self.handle_content_search_done(session),
            AppMsg::SearchInput(c) => self.handle_search_input(c),
            AppMsg::SearchBackspace => self.handle_search_backspace(&sender),
            AppMsg::SwitchHeader(view_name) => self.handle_switch_header(view_name),

            // Application Actions & Modals
            AppMsg::Open(position) => self.handle_open(position, &sender),
            AppMsg::Activate => self.handle_activate(&sender),
            AppMsg::LaunchWithApp(app_id) => self.handle_launch_with_app(app_id),
            AppMsg::ClearRecents => self.handle_clear_recents(&sender),
            AppMsg::PrepareContextMenu(x, y, path) => {
                self.handle_prepare_context_menu(x, y, path, &sender)
            }
            AppMsg::ShowContextMenu { x, y, path, mime } => {
                self.build_and_show_context_menu(x, y, path, mime, &sender)
            }
            AppMsg::ShowAbout => FluxApp::show_about_window(),
            AppMsg::ShowHelp => {
                let help_win = crate::ui::HelpWindow::builder().launch(()).detach();
                help_win.widget().present();
            }
            AppMsg::OpenFileProperties(path) => {
                let properties_win = FileProperties::builder().launch(path).detach();
                properties_win.widget().present();
            }
            AppMsg::PromptNewFolder => self.show_prompt_new_folder(&sender),
            AppMsg::PromptNewFile => self.show_prompt_new_file(&sender),
            AppMsg::PromptLocationDialog => {
                crate::ui::location_dialog::show_location_dialog(self, sender)
            }
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
            AppMsg::ShowIconPicker(target_path) => self.show_icon_picker(target_path, &sender),
            AppMsg::TriggerIconPicker => {
                let target = self
                    .get_selected_path()
                    .unwrap_or_else(|| self.current_path.clone());
                if target.is_dir() {
                    self.show_icon_picker(target, &sender);
                }
            }
            AppMsg::TriggerResetIcon => {
                let target = self
                    .get_selected_path()
                    .unwrap_or_else(|| self.current_path.clone());
                if target.is_dir() {
                    sender.input(AppMsg::ResetFolderIcon(target));
                }
            }

            // Remote Operations
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

            // Watcher Operations
            AppMsg::FileDeleted(path) => self.handle_file_deleted(path),
            AppMsg::FileChanged(path) => self.handle_file_changed(path, &sender),
            AppMsg::StartRename(path) => self.handle_start_rename(path),
            AppMsg::TriggerRenameSelection => self.handle_trigger_rename_selection(&sender),

            // Terminal Operations
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
            AppMsg::ToggleTerminal => self.handle_toggle_terminal(),

            // Task Operations
            AppMsg::TaskProgress {
                id,
                current,
                total,
                total_items,
                cancellable,
            } => self.handle_task_progress(id, current, total, total_items, cancellable),
            AppMsg::TaskCompleted(id) => self.handle_task_completed(id),
            AppMsg::CancelTask(id) => self.handle_cancel_task(id, &sender),
            AppMsg::CancelAllTasks => self.handle_cancel_all_tasks(&sender),
            AppMsg::TaskQueueTick => self.handle_task_queue_tick(&sender),
            AppMsg::Refresh => self.handle_refresh_path(&sender),
            AppMsg::RestoreItem(_) => {
                sender.input(AppMsg::Refresh);
            }
            AppMsg::ShowToast(msg) => {
                self.toast_overlay.add_toast(adw::Toast::new(&msg));
            }
        }
    }
}
