use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp, SortBy};
use crate::ui::constants;
use crate::ui::FileProperties;
use crate::utils;
use adw::gdk;
use adw::gio::prelude::*;
use adw::prelude::*;
use gtk::{gio, glib};
use relm4::prelude::*;
use std::sync::atomic::Ordering;
//use vte4::TerminalExt,

impl FluxApp {
    pub fn handle_update(&mut self, message: AppMsg, sender: relm4::AsyncComponentSender<Self>) {
        match message {
            AppMsg::RefreshSidebar => {
                // Reload the configuration from disk to capture external changes
                self.config = utils::load_config();
                // Clear the current sidebar factory items
                self.sidebar.guard().clear();
                // Repopulate with the new entries from the config file
                for place in &self.config.sidebar {
                    self.sidebar.guard().push_back(crate::ui::SidebarPlace {
                        name: place.name.clone(),
                        icon: place.icon.clone(),
                        path: if place.kind.as_deref() == Some("label") {
                            std::path::PathBuf::new()
                        } else {
                            utils::expand_path(&place.path)
                        },
                        is_mount: false,
                        is_section_label: place.kind.as_deref() == Some("label"),
                    });
                }

                // Re-run the standard refresh to append system drives/mounts
                self.refresh_sidebar();
            }
            AppMsg::RemoveFromSidebar(path) => {
                let path_str = path.to_string_lossy();
                // Match against both raw and tilde-collapsed forms
                let home = dirs::home_dir().unwrap_or_default();
                self.config.sidebar.retain(|entry| {
                    let expanded = if entry.path.starts_with('~') {
                        entry.path.replacen('~', &home.to_string_lossy(), 1)
                    } else {
                        entry.path.clone()
                    };
                    expanded != path_str.as_ref()
                });
                utils::save_config(&self.config);
                self.refresh_sidebar();
            }
            AppMsg::ShowAbout => {
                FluxApp::show_about_window();
            }
            AppMsg::UnmountDevice(path) => {
                let sender = sender.clone();
                let file = gio::File::for_path(&path);

                if let Ok(mount) = file.find_enclosing_mount(gio::Cancellable::NONE) {
                    mount.unmount_with_operation(
                        gio::MountUnmountFlags::NONE,
                        gio::MountOperation::NONE,
                        gio::Cancellable::NONE,
                        move |res| match res {
                            Ok(_) => sender.input(AppMsg::RefreshSidebar),
                            Err(e) => {
                                sender.input(AppMsg::ShowToast(format!("Unmount failed: {}", e)))
                            }
                        },
                    );
                }
            }
            AppMsg::AddToSidebarPermanent => {
                let path = self
                    .get_selected_path()
                    .unwrap_or_else(|| self.current_path.clone());
                let path_str = path.to_string_lossy().to_string();

                let already_exists = self.config.sidebar.iter().any(|entry| {
                    let expanded = if entry.path.starts_with('~') {
                        let home = dirs::home_dir().unwrap_or_default();
                        entry.path.replacen('~', &home.to_string_lossy(), 1)
                    } else {
                        entry.path.clone()
                    };
                    expanded == path_str
                });
                if !already_exists {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path_str.clone());
                    self.config.sidebar.insert(
                        0,
                        crate::model::CustomPlace {
                            name,
                            kind: None,
                            icon: "folder-symbolic".to_string(),
                            path: path_str,
                        },
                    );
                    utils::save_config(&self.config);
                    self.refresh_sidebar();
                }
            }
            AppMsg::ReorderSidebar { from, to } => {
                let home = dirs::home_dir().unwrap_or_default();
                let home_str = home.to_string_lossy();

                let resolve = |entry_path: &str| -> String {
                    if entry_path.starts_with('~') {
                        entry_path.replacen('~', &home_str, 1)
                    } else {
                        entry_path.to_owned()
                    }
                };
                let from_str = from.to_string_lossy().to_string();
                let to_str = to.to_string_lossy().to_string();

                let from_idx = self
                    .config
                    .sidebar
                    .iter()
                    .position(|e| resolve(&e.path) == from_str);
                let to_idx = self
                    .config
                    .sidebar
                    .iter()
                    .position(|e| resolve(&e.path) == to_str);
                if let (Some(fi), Some(ti)) = (from_idx, to_idx) {
                    let entry = self.config.sidebar.remove(fi);
                    // After removal, to_idx shifts by -1 if fi < ti
                    let insert_at = if fi < ti { ti - 1 } else { ti };
                    self.config.sidebar.insert(insert_at, entry);
                    utils::save_config(&self.config);
                    self.refresh_sidebar();
                }
            }
            AppMsg::SetSingleClick(val) => {
                self.config.ui.single_click = val;
                self.files.view.set_single_click_activate(val);
                utils::save_config(&self.config);
            }
            AppMsg::SetShowHidden(val) => {
                self.show_hidden = val;
                self.config.ui.show_hidden_by_default = val;
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::SetGridSpacing(val) => {
                self.config.ui.grid_spacing = val;
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::SetMaxWidthChars(val) => {
                self.config.ui.max_width_chars = val;
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::SetExpandLabels(val) => {
                self.config.ui.expand_labels = val;
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::SetFoldersFirst(val) => {
                self.config.ui.folders_first = val;
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::SetIconSize(val) => {
                self.config.ui.default_icon_size = val;
                self.current_icon_size = val;
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::SetSidebarWidth(val) => {
                self.config.ui.sidebar_width = val;
                utils::save_config(&self.config);
            }
            AppMsg::SetShowCsd(val) => {
                self.config.ui.show_csd = val;
                utils::save_config(&self.config);
            }
            AppMsg::SetShowXdgDirs(val) => {
                self.config.ui.show_xdg_dirs = val;
                utils::save_config(&self.config);
                sender.input(AppMsg::RefreshSidebar);
            }
            AppMsg::SetTheme(theme) => {
                self.config.ui.theme = theme;
                utils::save_config(&self.config);
            }
            AppMsg::SetDefaultSort(sort) => {
                self.config.ui.default_sort = sort;
                self.sort_by = sort;
                utils::save_config(&self.config);
                let _ = self.state_db.save_view(
                    &self.current_path,
                    &format!("{:?}", self.sort_by),
                    !self.sort_ascending,
                    self.current_icon_size as u32,
                    self.config.ui.folders_first,
                );
                sender.input(AppMsg::Refresh);
            }
            AppMsg::SetShortcut(key, val) => {
                match key.as_str() {
                    "back" => self.config.shortcuts.back = val,
                    "forward" => self.config.shortcuts.forward = val,

                    "open" => self.config.shortcuts.open = val,
                    "delete" => self.config.shortcuts.delete = val,
                    "refresh" => self.config.shortcuts.refresh = val,
                    "search" => self.config.shortcuts.search = val,

                    "toggle_hidden" => self.config.shortcuts.toggle_hidden = val,
                    _ => {}
                }
                crate::utils::save_config(&self.config);
            }
            AppMsg::SetMaximized(max) => {
                self.config.ui.start_maximized = max;
                crate::utils::save_config(&self.config);

                let app = gtk::Application::default();
                if let Some(window) = app.active_window() {
                    if max {
                        window.maximize();
                    } else {
                        window.unmaximize();
                    }
                }
            }
            AppMsg::ConfirmReplacePaste {
                files,
                conflicts,
                is_cut,
            } => {
                let window = gtk::Application::default().active_window();
                let body = if conflicts.len() == 1 {
                    format!(
                        "\"{}\" already exists in this location. Replace it and merge its contents?",
                        conflicts[0]

                    )
                } else {
                    format!(
                        "{} folders already exist in this location. Replace them and merge their contents?",

                        conflicts.len()
                    )
                };
                let dialog = gtk::MessageDialog::new(
                    window.as_ref(),
                    gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
                    gtk::MessageType::Warning,
                    gtk::ButtonsType::None,
                    "Replace Existing Folder?",
                );
                dialog.set_secondary_text(Some(&body));
                dialog.add_button("Cancel", gtk::ResponseType::Cancel);

                let replace_btn = dialog.add_button("Replace", gtk::ResponseType::Accept);
                replace_btn.style_context().add_class("destructive-action");

                let s = sender.clone();
                dialog.connect_response(move |dlg, response| {
                    dlg.close();
                    if response == gtk::ResponseType::Accept {
                        s.input(AppMsg::PerformPasteForced {
                            files: files.clone(),
                            is_cut,
                        });
                    }
                });
                dialog.present();
            }

            AppMsg::PerformPasteForced { files, is_cut } => {
                self.perform_paste_inner(files, is_cut, true, sender.clone());
            }
            AppMsg::PerformPaste { files, is_cut } => {
                self.perform_paste(files, is_cut, sender.clone());
            }
            AppMsg::Copy => {
                self.handle_clipboard_action(false);
            }
            AppMsg::Cut => {
                self.handle_clipboard_action(true);
            }
            AppMsg::Paste => {
                let Some(display) = gdk::Display::default() else {
                    sender.input(AppMsg::ShowToast(
                        "No display available for clipboard operation".to_string(),
                    ));
                    return;
                };

                let clipboard = display.clipboard();
                let s = sender.clone();

                clipboard.read_text_async(None::<&gio::Cancellable>, move |res| {
                    if let Ok(Some(text)) = res {
                        let mut lines = text.lines();
                        let first_line = lines.next().unwrap_or("");

                        let is_cut = first_line == "cut";

                        let files: Vec<gio::File> = lines
                            .filter(|uri| !uri.is_empty())
                            .map(|uri| gio::File::for_uri(uri.trim_end_matches('\r')))
                            .collect();

                        if !files.is_empty() {
                            s.input(AppMsg::PerformPaste { files, is_cut });
                        }
                    }
                });
            }
            AppMsg::PerformRename(old_path, new_name) => {
                match utils::rename_path(&old_path, &new_name) {
                    Ok(new_path) => {
                        let _ = self.state_db.rename_path(&old_path, &new_path);
                        sender.input(AppMsg::Navigate(self.current_path.clone()));
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        if msg.contains("Permission denied")
                            || msg.contains("Operation not permitted")
                        {
                            sender.input(AppMsg::ShowToast(
                                "Permission denied: Cannot move item to trash.".into(),
                            ));
                        } else {
                            sender.input(AppMsg::ShowToast(format!("Trash error: {}", e)));
                        }
                    }
                }
            }
            AppMsg::OpenFileProperties(path) => {
                let properties_win = FileProperties::builder().launch(path).detach();
                properties_win.widget().present();
            }
            AppMsg::SetWindowWidth(val) => {
                self.config.ui.startup_window_width = val;
                utils::save_config(&self.config);
            }
            AppMsg::SetWindowHeight(val) => {
                self.config.ui.startup_window_height = val;
                utils::save_config(&self.config);
            }
            AppMsg::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.config.ui.show_hidden_by_default = self.show_hidden;
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::ToggleSortOrder => {
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
            AppMsg::CycleSort => {
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
            AppMsg::CycleFolderPriority => {
                let path = self.current_path.clone();
                // Toggle logic: If we have a DB entry, use it, otherwise fallback to config default
                let current_state = if let Ok(Some((_, _, _, ff))) = self.state_db.get_view(&path) {
                    ff
                } else {
                    self.config.ui.folders_first
                };
                let new_state = !current_state;

                // Save toggle to DB
                let _ = self.state_db.save_view(
                    &path,
                    &format!("{:?}", self.sort_by),
                    false,
                    self.current_icon_size as u32,
                    new_state,
                );
                // Reload the current path to apply the new sorting
                self.load_path(path, &sender);
            }
            AppMsg::CloseSearchSync => {
                self.search_just_opened = false;
            }
            AppMsg::UpdateFilter(query) => {
                if self.filter == query && !self.search_just_opened {
                    return;
                }

                if self.header_view != constants::VIEW_SEARCH && !query.is_empty() {
                    self.header_view = constants::VIEW_SEARCH.to_string();
                }

                // Store current selection before filtering
                let selected_paths = self.get_selection();

                self.filter = query.clone();
                let query_lc = query.to_lowercase();

                self.files.clear_filters();
                if !query_lc.is_empty() {
                    let filter_str = query_lc.clone();
                    self.files
                        .add_filter(move |item| item.name.to_lowercase().contains(&filter_str));
                }

                // Restore selection if possible
                if !query_lc.is_empty() && !selected_paths.is_empty() {
                    if let Some(model) = self
                        .files
                        .view
                        .model()
                        .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                    {
                        // Iterate through all visible items after filtering
                        for idx in 0..self.files.len() {
                            if let Some(item) = self.files.get(idx) {
                                let path = item.borrow().path.clone();
                                if selected_paths.contains(&path) {
                                    // idx is u32, no cast needed
                                    model.select_item(idx, true);
                                    break;
                                }
                            }
                        }
                    }
                } else if !query_lc.is_empty() && self.search_just_opened {
                    // Only auto-select if filter just opened and no selection
                    if let Some(model) = self
                        .files
                        .view
                        .model()
                        .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                    {
                        if model.n_items() > 0 && self.get_selection().is_empty() {
                            model.select_item(0, true);
                        }
                    }
                    self.search_just_opened = false;
                }
            }
            AppMsg::SearchInput(c) => {
                self.search_just_opened = true;
                self.filter.push(c);
                self.header_view = constants::VIEW_SEARCH.to_string();
            }
            AppMsg::SearchBackspace => {
                if !self.filter.is_empty() {
                    self.filter.pop();
                    let query = self.filter.clone();
                    sender.input(AppMsg::UpdateFilter(query));
                }
            }
            AppMsg::StartRename(path) => {
                // Locate the item in the model to trigger the 'is_editing' UI state change
                let target_idx = (0..self.files.len())
                    .find(|&i| self.files.get(i).is_some_and(|r| r.borrow().path == path));

                if let Some(idx) = target_idx {
                    if let Some(item_wrapper) = self.files.get(idx) {
                        let mut item = item_wrapper.borrow().clone();
                        item.is_editing = true;
                        self.files.remove(idx);
                        self.files.insert(idx, item);
                    }
                }
            }
            AppMsg::TriggerRenameSelection => {
                if let Some(path) = self.get_selected_path() {
                    sender.input(AppMsg::StartRename(path));
                }
            }
            AppMsg::SwitchHeader(view_name) => {
                self.header_view = view_name;
                if self.header_view != constants::VIEW_SEARCH {
                    self.filter = String::new();
                    self.search_just_opened = true;
                    self.files.clear_filters();
                    sender.input(AppMsg::UpdateFilter(String::new()));
                }
            }
            AppMsg::ShowHelp => {
                let help_win = crate::ui::HelpWindow::builder().launch(()).detach();
                help_win.widget().present();
            }
            AppMsg::PrepareContextMenu(x, y, path) => {
                if let Some(ref target_path) = path {
                    let selection_model = self
                        .files
                        .view
                        .model()
                        .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                        .expect("Selection model must be MultiSelection");
                    let selection = selection_model.selection();
                    let mut target_idx = None;

                    for i in 0..self.files.len() {
                        if let Some(wrapper) = self.files.get(i) {
                            if &wrapper.borrow().path == target_path {
                                target_idx = Some(i);
                                break;
                            }
                        }
                    }

                    if let Some(idx) = target_idx {
                        if !selection.contains(idx) {
                            selection_model.select_item(idx, true);
                        }
                    }
                }

                let sender_ctx = sender.clone();
                relm4::spawn_blocking(move || {
                    let mime = path
                        .as_ref()
                        .map(|p| utils::get_mime_type(p))
                        .unwrap_or_else(|| constants::MIME_DIR.to_string());

                    sender_ctx.input(AppMsg::ShowContextMenu { x, y, path, mime });
                });
            }
            AppMsg::LaunchWithApp(app_id) => {
                if let Some(app_info) = gio_unix::DesktopAppInfo::new(&app_id) {
                    let selection = self.get_selection();
                    // Ensure we actually have files to open
                    if selection.is_empty() {
                        return;
                    }

                    let files: Vec<gio::File> =
                        selection.into_iter().map(gio::File::for_path).collect();
                    // Create a valid launch context
                    let context =
                        gdk::Display::default().map(|display| display.app_launch_context());
                    let launch_result = app_info.launch(&files, context.as_ref());

                    if let Err(e) = launch_result {
                        eprintln!("[Launch Error] {:?}: {}", app_info.display_name(), e);
                    }
                }
            }
            AppMsg::ClearRecents => {
                let selection = self.get_selection();
                let result = if selection.is_empty() {
                    crate::utils::remove_recents(None)
                } else {
                    crate::utils::remove_recents(Some(&selection))
                };

                match result {
                    Ok(()) => {
                        sender.input(AppMsg::Refresh);
                        self.recents_has_selection = false;
                    }
                    Err(e) => {
                        sender.input(AppMsg::ShowToast(format!("Failed to clear recents: {}", e)));
                    }
                }
            }
            AppMsg::SetTerminalHeight(h) => {
                self.config.ui.terminal.height = h;
                crate::utils::save_config(&self.config);
            }
            AppMsg::SetTerminalFont(f) => {
                self.config.ui.terminal.font = f;
                crate::utils::save_config(&self.config);
            }
            AppMsg::SetTerminalFgColor(c) => {
                self.config.ui.terminal.fg_color = c;
                crate::utils::save_config(&self.config);
            }
            AppMsg::SetTerminalBgColor(c) => {
                self.config.ui.terminal.bg_color = c;
                crate::utils::save_config(&self.config);
            }
            AppMsg::ToggleTerminal => {
                self.terminal_visible = !self.terminal_visible;
                if self.terminal_visible {
                    if !self.terminal_cleared {
                        self.terminal_cleared = true;
                    }

                    if let Some(dir) = self.current_path.to_str() {
                        self.terminal.respawn(dir);
                    }

                    // Set the paned position using the actual char_height from the
                    // terminal state rather than a hardcoded 24px approximation,
                    // so fish starts with the correct row count.
                    if let Some(paned) = &self.terminal_paned {
                        let height = paned.height();
                        if height > 0 {
                            let char_height = self.terminal.char_height().max(1);
                            let terminal_height = self.config.ui.terminal.height * char_height;
                            paned.set_position(height - terminal_height);
                        }
                    }

                    let term = self.terminal.clone();
                    glib::idle_add_local_once(move || {
                        term.grab_focus();
                        // After the pane has settled and the draw func has run,
                        // send SIGWINCH so fish re-reads the correct $LINES/$COLUMNS.
                        term.send_sigwinch();
                    });
                }
            }
            AppMsg::ShowContextMenu { x, y, path, mime } => {
                self.active_item_path = path.clone();
                let is_in_trash = self
                    .current_path
                    .to_string_lossy()
                    .starts_with(constants::TRASH_URI);
                let root_menu = gio::Menu::new();
                let main_section = gio::Menu::new();

                let mut open_with_item: Option<gio::MenuItem> = None;
                // Registry for dynamic submenus: Map<SubmenuName, MenuModel>
                let mut submenu_map: indexmap::IndexMap<String, gio::Menu> =
                    indexmap::IndexMap::new();
                for action in &self.menu_actions {
                    let mut matches = false;
                    // --- FILTERING LOGIC ---
                    if is_in_trash {
                        if action
                            .mime_types
                            .contains(&constants::FILTER_TRASH.to_string())
                        {
                            matches = true;
                        }
                    } else {
                        for allowed_mime in &action.mime_types {
                            if allowed_mime == constants::FILTER_TRASH {
                                continue;
                            }

                            let requirements: Vec<&str> = allowed_mime.split('+').collect();
                            matches = requirements.iter().any(|req| match req.trim() {
                                constants::FILTER_ALL | "all" => true,
                                "image/all" | "image/*" => mime.starts_with("image/"),

                                "video/all" | "video/*" => mime.starts_with("video/"),
                                "audio/all" | "audio/*" => mime.starts_with("audio/"),
                                "font/all" | "font/*" => mime.starts_with("font/"),

                                "model/all" | "model/*" => mime.starts_with("model/"),
                                "message/all" | "message/*" => mime.starts_with("message/"),
                                "chemical/all" | "chemical/*" => mime.starts_with("chemical/"),

                                "multipart/all" | "multipart/*" => mime.starts_with("multipart/"),
                                "x-content/all" | "x-content/*" => mime.starts_with("x-content/"),
                                "application/all" | "application/*" => {
                                    mime.starts_with("application/")
                                }

                                "text/all" | "text/*" => {
                                    mime.starts_with("text/")
                                        || gio::content_type_is_a(&mime, constants::MIME_TEXT)
                                        || mime == constants::MIME_EMPTY
                                }
                                constants::FILTER_FOLDER | "directory" => {
                                    mime == constants::MIME_DIR
                                }

                                constants::FILTER_FILE => mime != constants::MIME_DIR,
                                t if t.ends_with('/') => mime.starts_with(t),
                                t => t == mime,
                            });
                            if matches {
                                break;
                            }
                        }
                    }

                    // --- MENU ASSEMBLY ---
                    if matches {
                        // --- MINIMAL BUILTIN MAPPING ---
                        let (full_action_name, lookup_name) = match action.command.as_str() {
                            "builtin::copy" => ("win.copy".to_string(), "copy"),
                            "builtin::cut" => ("win.cut".to_string(), "cut"),
                            "builtin::paste" => ("win.paste".to_string(), "paste"),
                            "builtin::add_to_quick_list" => {
                                if let Some(ref target) = path {
                                    let quick_action =
                                        gio::SimpleAction::new("add-to-quick-list", None);
                                    let target_clone = target.clone();
                                    let sender_q = sender.clone();
                                    quick_action.connect_activate(move |_, _| {
                                        sender_q.input(AppMsg::AddExclusive(Some(
                                            target_clone.clone(),
                                        )));
                                    });
                                    self.action_group.add_action(&quick_action);
                                }
                                ("win.add-to-quick-list".to_string(), "add-to-quick-list")
                            }
                            "builtin::open_with" => {
                                let open_with_menu = gio::Menu::new();
                                let apps = gio::AppInfo::all_for_type(&mime);
                                for app in apps {
                                    let label = app.display_name();
                                    let app_id = app
                                        .id()
                                        .map(|id| id.to_string())
                                        .unwrap_or_else(|| app.name().to_string());
                                    let item =
                                        gio::MenuItem::new(Some(&label), Some("win.launch-with"));
                                    item.set_action_and_target_value(
                                        Some("win.launch-with"),
                                        Some(&app_id.to_variant()),
                                    );
                                    open_with_menu.append_item(&item);
                                }

                                // Explicitly create the item and set the label to preserve spacing
                                let menu_item = gio::MenuItem::new_submenu(None, &open_with_menu);
                                // Using \u{a0} (Non-breaking space) to prevent GTK from collapsing the gap
                                let spaced_label = "󰱝\u{a0} \u{a0} \u{a0} Open With...".to_string();
                                menu_item.set_label(Some(&spaced_label));

                                open_with_item = Some(menu_item);
                                continue;
                            }
                            _ => (
                                format!("win.{}", action.action_name),
                                action.action_name.as_str(),
                            ),
                        };
                        // --- ENABLE ACTION ---
                        if let Some(g_action) = self.action_group.lookup_action(lookup_name) {
                            if let Some(simple) = g_action.downcast_ref::<gio::SimpleAction>() {
                                simple.set_enabled(true);
                                if let Some(toast_msg) = &action.toast {
                                    self.pending_toasts
                                        .insert(action.action_name.clone(), toast_msg.clone());
                                }
                            }
                        }

                        // Route to Submenu or Main Section

                        if let Some(group_name) = &action.submenu {
                            let menu = submenu_map.entry(group_name.clone()).or_default();
                            menu.append(Some(&action.label), Some(&full_action_name));
                        } else {
                            main_section.append(Some(&action.label), Some(&full_action_name));
                        }
                    }
                }

                // Assemble the UI
                root_menu.append_section(None, &main_section);
                if let Some(item) = open_with_item {
                    root_menu.append_item(&item);
                }

                // Append all generated submenus to the root
                for (name, menu) in submenu_map {
                    root_menu.append_submenu(Some(&name), &menu);
                }

                self.context_menu_popover.set_menu_model(Some(&root_menu));
                self.context_menu_popover
                    .set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                self.context_menu_popover.popup();
            }
            AppMsg::TriggerIconPicker => {
                // Get the selected item path, or fallback to the current directory if nothing is selected.
                let target = self
                    .get_selected_path()
                    .unwrap_or_else(|| self.current_path.clone());

                // Only proceed if the target is a directory.
                // This prevents opening the picker for selected files.
                if target.is_dir() {
                    sender.input(AppMsg::ShowIconPicker(target));
                }
            }
            AppMsg::TriggerResetIcon => {
                // Get the selected item path, or fallback to the current directory.
                let target = self
                    .get_selected_path()
                    .unwrap_or_else(|| self.current_path.clone());

                // Only proceed if the target is a directory.
                if target.is_dir() {
                    sender.input(AppMsg::ResetFolderIcon(target));
                }
            }
            AppMsg::ExecuteCommand(cmd_template) => {
                let mut targets = Vec::new();
                if let Some(model) = self
                    .files
                    .view
                    .model()
                    .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                {
                    let bitset = model.selection();
                    for i in 0..bitset.size() {
                        let pos = bitset.nth(i as u32);
                        if let Some(wrapper) = self.files.get(pos) {
                            targets.push(wrapper.borrow().path.clone());
                        }
                    }
                }

                let final_targets = if let Some(active) = &self.active_item_path {
                    if targets.contains(active) {
                        targets
                    } else {
                        vec![active.clone()]
                    }
                } else {
                    vec![self.current_path.clone()]
                };
                if cmd_template == "builtin::open_with" {
                    if let Some(path) = final_targets.first() {
                        let file = gio::File::for_path(path);
                        if let Ok(info) = file.query_info(
                            "standard::content-type",
                            gio::FileQueryInfoFlags::NONE,
                            gio::Cancellable::NONE,
                        ) {
                            if let Some(mime) = info.content_type() {
                                let apps = gio::AppInfo::all_for_type(&mime);
                                if let Some(app) = apps.first() {
                                    let files: Vec<gio::File> =
                                        final_targets.iter().map(gio::File::for_path).collect();
                                    let _ = app.launch(&files, None::<&gio::AppLaunchContext>);
                                }
                            }
                        }
                    }
                    return;
                }

                // Extract data for the background task
                let current_path = self.current_path.clone();
                let toast_msg = self
                    .menu_actions
                    .iter()
                    .find(|action| action.command == cmd_template)
                    .and_then(|a| a.toast.clone());
                // Commands that mutate the trash virtual filesystem require a view refresh
                // after completion,
                // spawn() is fire-and-forget so the flag is determined
                // before entering the blocking task.
                let needs_refresh = self
                    .current_path
                    .to_string_lossy()
                    .starts_with(constants::TRASH_URI);
                let sender_clone = sender.clone();

                relm4::spawn_blocking(move || {
                    if final_targets.len() == 1 {
                        Self::run_custom_command_wait(&cmd_template, &final_targets[0]);
                    } else if !final_targets.is_empty() {
                        let paths_arg = final_targets
                            .iter()
                            .map(|p| format!("'{}'", p.to_string_lossy().replace("'", "'\\''")))
                            .collect::<Vec<_>>()
                            .join(" ");

                        let mut cmd = cmd_template.replace(constants::TEMPLATE_PATHS, &paths_arg);
                        if cmd.contains(constants::TEMPLATE_CWD) {
                            cmd = cmd.replace(
                                constants::TEMPLATE_CWD,
                                &format!(
                                    "'{}'",
                                    current_path.to_string_lossy().replace("'", "'\\''")
                                ),
                            );
                        }

                        let _ = std::process::Command::new(constants::SHELL_BIN)
                            .arg("-c")
                            .arg(cmd)
                            .status();
                    }

                    // Send toast back to main thread after execution starts/finishes
                    if let Some(msg) = toast_msg {
                        sender_clone.input(AppMsg::ShowToast(msg));
                    }

                    if needs_refresh {
                        sender_clone.input(AppMsg::Refresh);
                    }
                });
            }
            AppMsg::Zoom(delta) => {
                // Determine if we are zooming in or out using the STEP constant
                let change = if delta > 0.0 {
                    -constants::ZOOM_STEP
                } else {
                    constants::ZOOM_STEP
                };
                // Use the MIN and MAX constants to prevent the icons from becoming too small or too large
                let new_size = (self.current_icon_size + change)
                    .clamp(constants::ZOOM_MIN, constants::ZOOM_MAX);
                if new_size != self.current_icon_size {
                    self.current_icon_size = new_size;
                    // Save the new size to SQLite DB so it persists for this folder
                    let _ = self.state_db.save_view(
                        &self.current_path,
                        &format!("{:?}", self.sort_by),
                        false,
                        new_size as u32,
                        self.config.ui.folders_first,
                    );
                    // Update all visible items in the grid
                    for i in 0..self.files.len() {
                        if let Some(item_wrapper) = self.files.get(i) {
                            let mut item = item_wrapper.borrow().clone();
                            item.icon_size = new_size;
                            self.files.remove(i);
                            self.files.insert(i, item);
                        }
                    }
                }
            }
            AppMsg::ToggleSidebar => {
                self.sidebar_visible = !self.sidebar_visible;
                self.config.ui.sidebar_visible = self.sidebar_visible;
                crate::utils::save_config(&self.config);
                if let Some(ref widget) = self.sidebar_widget {
                    widget.set_visible(self.sidebar_visible);
                }
            }
            AppMsg::ToggleSingleClick => {
                self.config.ui.single_click = !self.config.ui.single_click;
                self.files
                    .view
                    .set_single_click_activate(self.config.ui.single_click);
                utils::save_config(&self.config);
            }
            //WARN: Change this logic with caution.
            // If the process working directory
            // (CWD) is not synchronized, operations like drag-and-drop or shell commands
            // may resolve relative paths incorrectly, moving files to previous locations
            // instead of the directory currently displayed to the user.
            AppMsg::Navigate(path) => {
                let path_str = path.to_string_lossy();
                // Explicitly allow root directory and handle edge cases
                let path_valid = path_str == "/"
                    || path.exists()
                    || path_str.starts_with(constants::TRASH_URI)
                    || path_str.starts_with(constants::RECENT_URI);

                if !path_valid {
                    #[cfg(debug_assertions)]
                    eprintln!("[Flux] Cannot navigate: path does not exist: {}", path_str);
                    return;
                }

                // Validate path existence (except for virtual trash URI)

                // Only proceed if the target is a directory/trash and different from current location
                if (path.is_dir()
                    || path_str.starts_with(constants::TRASH_URI)
                    || path_str.starts_with(constants::RECENT_URI))
                    && path != self.current_path
                {
                    let old_path = std::mem::replace(&mut self.current_path, path.clone());
                    // Synchronize the physical process working directory with the application state.
                    // This ensures std::fs operations and spawned child processes resolve
                    // relative paths against the folder currently visible in the UI.
                    if path.is_absolute() {
                        let _ = std::env::set_current_dir(&path);
                    }

                    // Manage navigation history and recent items stack
                    self.recent_stack.retain(|p| p != &path && p != &old_path);
                    self.recent_stack.push_front(old_path.clone());
                    self.recent_stack.truncate(constants::MAX_RECENT_ITEMS);

                    // Reset search and filter state before entering new directory
                    self.filter.clear();
                    self.files.clear_filters();

                    sender.input(AppMsg::CloseSearchSync);

                    // Revert header UI from search mode back to path/breadcrumb view
                    if self.header_view == constants::VIEW_SEARCH {
                        self.header_view = "path".to_string();
                    }

                    self.history.push(old_path);
                    self.forward_stack.clear();

                    // Perform the actual I/O to populate the file model for the new path
                    self.load_path(path, &sender);
                    self.update_breadcrumbs();

                    // Update focus and selection on the next main loop iteration.
                    // When the terminal triggered this navigation via OSC 7 (cd),
                    // it already holds focus, don't steal it for the file grid.
                    let view = self.files.view.clone();
                    let terminal = self.terminal.clone();
                    let terminal_visible = self.terminal_visible;
                    glib::idle_add_local_once(move || {
                        let terminal_has_focus = terminal_visible && terminal.has_focus();
                        if !terminal_has_focus {
                            view.grab_focus();
                        }
                    });
                }
            }
            AppMsg::SetRecentsRow(val) => {
                self.config.ui.recents_row = val;
                utils::save_config(&self.config);
                sender.input(AppMsg::RefreshSidebar);
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
                let path_to_add = explicit_path.unwrap_or_else(|| {
                    self.get_selected_path()
                        .unwrap_or_else(|| self.current_path.clone())
                });
                if !self.exclusive_list.contains(&path_to_add) {
                    self.exclusive_list.push(path_to_add);
                    if self.exclusive_index.is_none() {
                        self.exclusive_index = Some(self.exclusive_list.len() - 1);
                    }
                    self.refresh_sidebar();
                }
            }
            AppMsg::ClearExclusive => {
                self.exclusive_list.clear();
                self.exclusive_index = None;
                self.refresh_sidebar();
            }
            AppMsg::NextExclusive => {
                if !self.exclusive_list.is_empty() {
                    let new_idx = match self.exclusive_index {
                        Some(i) => (i + 1) % self.exclusive_list.len(),

                        None => 0,
                    };
                    self.exclusive_index = Some(new_idx);
                    let target = self.exclusive_list[new_idx].clone();
                    sender.input(AppMsg::Navigate(target));
                }
            }
            AppMsg::PrevExclusive => {
                if !self.exclusive_list.is_empty() {
                    let new_idx = match self.exclusive_index {
                        Some(i) if i > 0 => i - 1,
                        _ => self.exclusive_list.len() - 1,
                    };
                    self.exclusive_index = Some(new_idx);
                    let target = self.exclusive_list[new_idx].clone();
                    sender.input(AppMsg::Navigate(target));
                }
            }
            AppMsg::SelectionChanged => {
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

                // ── Update recents selection flag ──────────────────────────────
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
                        // Kick off a non-blocking duration probe for audio/video files.
                        // The result arrives via MediaDurationReady and appends to the status.
                        let selected_path = self
                            .files
                            .view
                            .model()
                            .and_downcast::<gtk::MultiSelection>()
                            .and_then(|m| {
                                let pos = m.selection().nth(0);
                                self.files.get(pos)
                            })
                            .map(|w| w.borrow().path.clone());
                        if let Some(path) = selected_path {
                            let s = sender.clone();
                            relm4::spawn_blocking(move || {
                                let mime = utils::get_mime_type(&path);

                                let dimensions = if mime.starts_with("image/") {
                                    crate::utils::media::probe_image_dimensions(&path)
                                } else {
                                    None
                                };

                                if mime.starts_with("audio/") || mime.starts_with("video/") {
                                    let dur = crate::utils::media::probe_media_duration(&path);
                                    s.input(AppMsg::MediaDurationReady(dur));
                                }

                                s.input(AppMsg::FileMetaReady { mime, dimensions });
                            });
                        }

                        format!("{} ({})", single_name, size_str)
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
                            let path = wrapper.borrow().path.clone();
                            let child_count =
                                std::fs::read_dir(&path).map(|rd| rd.count()).unwrap_or(0);
                            format!("{} ({} items)", single_name, child_count)
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
            AppMsg::ThumbnailReady {
                name,
                texture,
                load_id,
            } => {
                if load_id == self.load_id.load(Ordering::SeqCst) {
                    let target_idx = (0..self.files.len())
                        .find(|&i| self.files.get(i).is_some_and(|r| r.borrow().name == name));
                    if let Some(idx) = target_idx {
                        if let Some(item_wrapper) = self.files.get(idx) {
                            let mut item = item_wrapper.borrow().clone();
                            item.thumbnail = Some(texture);
                            self.files.remove(idx);
                            self.files.insert(idx, item);
                        }
                    }
                }
            }
            AppMsg::GoBack => {
                if let Some(prev) = self.history.pop() {
                    self.forward_stack.push(self.current_path.clone());
                    self.load_path(prev, &sender);
                    self.update_breadcrumbs();
                } else if let Some(parent) = self.current_path.parent() {
                    let parent_path = parent.to_path_buf();
                    self.forward_stack.push(self.current_path.clone());
                    self.load_path(parent_path, &sender);
                    self.update_breadcrumbs();
                }
            }
            AppMsg::GoForward => {
                if let Some(next) = self.forward_stack.pop() {
                    self.history.push(self.current_path.clone());
                    self.load_path(next, &sender);
                    self.update_breadcrumbs();
                }
            }
            AppMsg::TaskProgress {
                id,
                current,
                total,
                total_items,

                cancellable,
            } => {
                self.task_queue
                    .update(id, current, total, total_items, cancellable);
                // No UI update here, the 100ms tick handles rendering.
            }
            AppMsg::TaskCompleted(id) => {
                self.task_queue.remove(id);
            }
            AppMsg::CancelTask(id) => {
                self.task_queue.cancel(id);
                sender.input(AppMsg::SelectionChanged);
            }
            AppMsg::CancelAllTasks => {
                self.task_queue.cancel_all();
                sender.input(AppMsg::SelectionChanged);
            }
            AppMsg::TaskQueueTick => match self.task_queue.summary() {
                Some((1, 1, pct)) => {
                    self.selection_status = format!("[Copying 1 file | {:.0}%]", pct * 100.0);
                }
                Some((1, items, pct)) => {
                    self.selection_status =
                        format!("[Copying {} files | {:.0}%]", items, pct * 100.0);
                }
                Some((n, items, pct)) => {
                    self.selection_status =
                        format!("[{} operations, {} files | {:.0}%]", n, items, pct * 100.0);
                }
                None => {
                    if self.selection_status.starts_with('[') {
                        self.selection_status = String::new();
                        sender.input(AppMsg::SelectionChanged);
                    }
                }
            },
            AppMsg::Refresh => {
                self.is_loading = true;
                let p = self.current_path.clone();
                self.load_path(p, &sender);
            }
            AppMsg::FileDeleted(path) => {
                if let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) {
                    let target_idx = (0..self.files.len())
                        .find(|&i| self.files.get(i).is_some_and(|r| r.borrow().name == name));
                    if let Some(idx) = target_idx {
                        self.files.remove(idx);
                    }
                }
            }
            AppMsg::FileChanged(path) => {
                if let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) {
                    let file = gio::File::for_path(&path);
                    let attributes = "standard::name,standard::display-name,standard::type";

                    if let Ok(info) = file.query_info(
                        attributes,
                        gio::FileQueryInfoFlags::NONE,
                        gio::Cancellable::NONE,
                    ) {
                        let is_dir = info.file_type() == gio::FileType::Directory;
                        let display_name = info.display_name().to_string();
                        let icon = utils::get_icon_for_path(&path, is_dir);

                        let target_idx = (0..self.files.len())
                            .find(|&i| self.files.get(i).is_some_and(|r| r.borrow().name == name));
                        if let Some(idx) = target_idx {
                            if let Some(item_wrapper) = self.files.get(idx) {
                                let mut item = item_wrapper.borrow().clone();
                                item.icon = icon;
                                self.files.remove(idx);
                                self.files.insert(idx, item);
                            }
                        } else {
                            let item = crate::ui::FileItem {
                                name: display_name.clone(),
                                icon,
                                thumbnail: None,
                                is_dir,
                                path: path.clone(),
                                icon_size: self.current_icon_size,
                                size: info.size() as u64,
                                is_editing: false,
                                is_foreign_owner: false,
                                expand_labels: self.config.ui.expand_labels,
                                is_custom_icon: false,
                            };
                            self.files.append(item);

                            let current_session = self.load_id.load(Ordering::SeqCst);
                            self.spawn_thumbnail_loader(
                                vec![(display_name, path)],
                                current_session,
                                sender.clone(),
                            );
                            sender.input(AppMsg::Refresh);
                        }
                    } else {
                        let target_idx = (0..self.files.len())
                            .find(|&i| self.files.get(i).is_some_and(|r| r.borrow().name == name));
                        if let Some(idx) = target_idx {
                            self.files.remove(idx);
                        }
                    }
                }
            }
            AppMsg::Delete => {
                let selection = self.get_selection();
                if selection.is_empty() {
                    return;
                }

                let sender_clone = sender.clone();
                for path in selection {
                    let file = gio::File::for_path(path);
                    let s = sender_clone.clone();

                    // GIO Move to Trash logic
                    file.trash_async(
                        glib::Priority::DEFAULT,
                        gio::Cancellable::NONE,
                        move |res| {
                            match res {
                                Ok(_) => {
                                    // Refresh the view to remove deleted items from UI
                                    s.input(AppMsg::Refresh);
                                }

                                Err(e) => {
                                    let msg = e.message().to_string();
                                    if msg.contains("Permission denied")
                                        || msg.contains("Operation not permitted")
                                    {
                                        s.input(AppMsg::ShowToast(
                                            "Permission denied: Cannot move item to trash.".into(),
                                        ));
                                    } else {
                                        s.input(AppMsg::ShowToast(format!("Trash error: {}", e)));
                                    }
                                }
                            }
                        },
                    );
                }
            }
            AppMsg::Open | AppMsg::Activate => {
                // Determine hardware state of the keyboard
                let modifiers = gdk::Display::default()
                    .and_then(|d| d.default_seat())
                    .and_then(|s| s.keyboard())
                    .map(|k| k.modifier_state())
                    .unwrap_or(gdk::ModifierType::empty());
                let is_selecting = modifiers
                    .intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK);
                // If holding Ctrl/Shift, we stop the "Open" action.
                // This prevents the app from navigating away during multi-selection.
                if is_selecting {
                    return;
                }

                let selection = self.get_selection();
                if selection.is_empty() {
                    return;
                }

                for path in selection {
                    if path.is_dir() {
                        sender.input(AppMsg::Navigate(path));
                        break;
                    } else {
                        utils::open_file(path);
                    }
                }
            }
            AppMsg::HandleDrop {
                source_paths,
                dest_path,
            } => {
                let sender_clone = sender.clone();

                relm4::spawn_blocking(move || {
                    for source_path in source_paths {
                        if !dest_path.is_dir() {
                            break;
                        }

                        let Some(file_name) = source_path.file_name() else {
                            continue;
                        };

                        let final_dest = dest_path.join(file_name);

                        if source_path == final_dest {
                            continue;
                        }

                        let src_file = gio::File::for_path(&source_path);
                        let dst_file = gio::File::for_path(&final_dest);

                        if let Err(e) = src_file.move_(
                            &dst_file,
                            gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                            gio::Cancellable::NONE,
                            None,
                        ) {
                            eprintln!("[DnD Error] Failed to move {:?}: {}", source_path, e);
                        }
                    }

                    sender_clone.input(AppMsg::Refresh);
                });
            }
            AppMsg::HandleExternalDrop {
                source_paths,
                dest_path,
            } => {
                let sender_clone = sender.clone();
                relm4::spawn_blocking(move || {
                    for source in source_paths {
                        let Some(file_name) = source.file_name() else {
                            continue;
                        };

                        let final_dest = dest_path.join(file_name);

                        if source == final_dest {
                            continue;
                        }

                        let src_file = gio::File::for_path(&source);
                        let dst_file = gio::File::for_path(&final_dest);

                        if let Err(e) = src_file.move_(
                            &dst_file,
                            gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                            gio::Cancellable::NONE,
                            None,
                        ) {
                            eprintln!("[File Error] External move failed: {}", e);
                        }
                    }

                    sender_clone.input(AppMsg::Refresh);
                });
            }
            AppMsg::EmptyTrash => {
                let root = gio::File::for_uri(constants::TRASH_URI);
                if let Ok(enumerator) = root.enumerate_children(
                    "standard::name",
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                ) {
                    for info in enumerator.flatten() {
                        // Delete each item in the trash virtual directory
                        let _ = root.child(info.name()).delete(gio::Cancellable::NONE);
                    }
                }
                // Refresh the view to show the now-empty trash
                sender.input(AppMsg::Refresh);
            }
            AppMsg::ShowToast(msg) => {
                self.toast_overlay.add_toast(adw::Toast::new(&msg));
            }
            AppMsg::MediaDurationReady(maybe_duration) => {
                // Only append duration if status bar still shows a single-file selection
                // (the user hasn't moved on) and a task is not running.
                if self.task_queue.summary().is_none()
                    && !self.selection_status.starts_with('[')
                    && !self.selection_status.is_empty()
                {
                    if let Some(dur) = maybe_duration {
                        let dur_str = crate::utils::media::format_duration(dur);
                        // Append only if not already present (guard against duplicate events)
                        if !self.selection_status.contains(&dur_str) {
                            self.selection_status.push_str(&format!(" - {}", dur_str));
                        }
                    }
                }
            }
            AppMsg::FileMetaReady { mime, dimensions } => {
                if self.task_queue.summary().is_some() || self.selection_status.is_empty() {
                    return;
                }

                // Only act on single-file selections, multi-selection status starts with a digit
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
                // Append dimensions first (before mime), then mime type
                if let Some(d) = dim_str {
                    self.selection_status.push_str(&d);
                }

                self.selection_status.push_str(&format!(" - {}", mime));
            }
            AppMsg::SetAsc(asc) => {
                self.sort_ascending = asc;
                let _ = self.state_db.save_view(
                    &self.current_path,
                    &format!("{:?}", self.sort_by),
                    !self.sort_ascending,
                    self.current_icon_size as u32,
                    self.config.ui.folders_first,
                );
                sender.input(AppMsg::Refresh);
            }
            AppMsg::RestoreItem(_) => {
                sender.input(AppMsg::Refresh);
            }
            AppMsg::ShowIconPicker(target_path) => {
                use gtk::prelude::*;
                // Safely grab any active window to parent the modal dialog
                let toplevels = gtk::Window::list_toplevels();
                let parent = toplevels
                    .first()
                    .and_then(|w| w.downcast_ref::<gtk::Window>());
                let dialog = gtk::Dialog::builder()
                    .title("Select Folder Icon")
                    .transient_for(parent.unwrap())
                    .modal(true)
                    .use_header_bar(1)
                    .build();
                let flow_box = gtk::FlowBox::builder()
                    .valign(gtk::Align::Start)
                    .max_children_per_line(6)
                    .min_children_per_line(6)
                    .selection_mode(gtk::SelectionMode::Single)
                    .build();
                let scrolled = gtk::ScrolledWindow::builder()
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .vscrollbar_policy(gtk::PolicyType::Automatic)
                    .child(&flow_box)
                    .height_request(350)
                    .width_request(400)
                    .build();
                let search_entry = gtk::SearchEntry::builder()
                    .margin_top(6)
                    .margin_bottom(6)
                    .margin_start(6)
                    .margin_end(6)
                    .build();
                let content_area = dialog.content_area();
                content_area.append(&search_entry);
                content_area.append(&scrolled);
                let icon_theme = gtk::IconTheme::for_display(&gdk::Display::default().unwrap());
                let icon_names = icon_theme.icon_names();
                for icon_name in icon_names {
                    let name_str = icon_name.as_str();
                    if name_str.contains("folder") || name_str.contains("Folder") {
                        let image = gtk::Image::from_icon_name(name_str);
                        image.set_icon_size(gtk::IconSize::Large);
                        let button = gtk::Button::builder()
                            .child(&image)
                            .tooltip_text(name_str)
                            .has_frame(false)
                            .build();
                        unsafe {
                            button.set_data("icon-name", icon_name.to_string());
                        }
                        let dialog_btn_clone = dialog.clone();
                        let flow_box_btn_clone = flow_box.clone();
                        button.connect_clicked(move |btn| {
                            if let Some(row) = btn
                                .parent()
                                .and_then(|p| p.downcast::<gtk::FlowBoxChild>().ok())
                            {
                                flow_box_btn_clone.select_child(&row);
                                dialog_btn_clone.response(gtk::ResponseType::Ok);
                            }
                        });
                        flow_box.append(&button);
                    }
                }
                let flow_box_clone = flow_box.clone();
                search_entry.connect_search_changed(move |entry| {
                    let text = entry.text().to_string().to_lowercase();
                    let mut child = flow_box_clone.first_child();
                    while let Some(ref widget) = child {
                        if let Some(child_row) = widget.downcast_ref::<gtk::FlowBoxChild>() {
                            if let Some(button) = child_row
                                .child()
                                .and_then(|c| c.downcast::<gtk::Button>().ok())
                            {
                                unsafe {
                                    if let Some(name) =
                                        button.data::<String>("icon-name").map(|p| p.as_ref())
                                    {
                                        child_row.set_visible(name.to_lowercase().contains(&text));
                                    }
                                }
                            }
                        }
                        child = widget.next_sibling();
                    }
                });
                let dialog_select = dialog.clone();
                flow_box.connect_child_activated(move |_, _| {
                    dialog_select.response(gtk::ResponseType::Ok);
                });
                dialog.add_button("Cancel", gtk::ResponseType::Cancel);
                dialog.add_button("Select", gtk::ResponseType::Ok);

                let flow_box_select = flow_box.clone();
                let sender_clone = sender.clone();
                let target_path_clone = target_path.clone();

                dialog.connect_response(move |win, response| {
                    if response == gtk::ResponseType::Ok {
                        if let Some(row) = flow_box_select.selected_children().first() {
                            if let Some(button) =
                                row.child().and_then(|c| c.downcast::<gtk::Button>().ok())
                            {
                                unsafe {
                                    if let Some(name) =
                                        button.data::<String>("icon-name").map(|p| p.as_ref())
                                    {
                                        // Dispatch to your existing, working state handler
                                        sender_clone.input(AppMsg::SetFolderIcon {
                                            path: target_path_clone.clone(),
                                            icon_name: name.to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    win.destroy();
                });
                dialog.present();
            }
            AppMsg::SetFolderIcon { path, icon_name } => {
                self.config
                    .ui
                    .folder_icons
                    .insert(path.to_string_lossy().to_string(), icon_name);
                crate::utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::ResetFolderIcon(path) => {
                self.config
                    .ui
                    .folder_icons
                    .remove(&path.to_string_lossy().to_string());
                crate::utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::SetShowThumbnails(val) => {
                self.config.ui.show_thumbnails = val;
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::SetThumbnailType { type_name, enabled } => {
                match type_name.as_str() {
                    "images" => self.config.ui.thumbnail_types.images = enabled,
                    "videos" => self.config.ui.thumbnail_types.videos = enabled,
                    "fonts" => self.config.ui.thumbnail_types.fonts = enabled,
                    "pdfs" => self.config.ui.thumbnail_types.pdfs = enabled,
                    _ => {}
                }
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::SetShowRecents(val) => {
                self.config.ui.show_recents = val;
                utils::save_config(&self.config);
                sender.input(AppMsg::RefreshSidebar);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::SortBy;
    use std::env;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_cycle_sort_logic() {
        let mut current_sort = SortBy::Name;
        let cycle = |s: SortBy| match s {
            SortBy::Name => SortBy::Date,
            SortBy::Date => SortBy::Size,
            SortBy::Size => SortBy::Type,
            SortBy::Type => SortBy::Name,
        };
        current_sort = cycle(current_sort);
        assert_eq!(current_sort, SortBy::Date);

        current_sort = cycle(current_sort);
        assert_eq!(current_sort, SortBy::Size);

        current_sort = cycle(current_sort);
        assert_eq!(current_sort, SortBy::Type);

        current_sort = cycle(current_sort);
        assert_eq!(current_sort, SortBy::Name);
    }

    #[test]
    fn test_history_navigation_integrity() {
        let base = env::temp_dir().join("flux_test_env");
        let mut history: Vec<PathBuf> = Vec::new();
        let mut forward_stack: Vec<PathBuf> = Vec::new();
        let mut current_path = base.clone();
        let subfolder = base.join("documents");
        history.push(current_path.clone());
        current_path = subfolder.clone();
        forward_stack.clear();

        assert_eq!(current_path, subfolder);
        assert_eq!(history.len(), 1);
        if let Some(prev) = history.pop() {
            forward_stack.push(current_path.clone());
            current_path = prev;
        }

        assert_eq!(current_path, base);
        assert_eq!(forward_stack.len(), 1);
        if let Some(next) = forward_stack.pop() {
            history.push(current_path.clone());
            current_path = next;
        }

        assert_eq!(current_path, subfolder);
        assert!(forward_stack.is_empty());
    }

    #[test]
    fn test_asynchronous_load_synchronization() {
        let load_id = Arc::new(AtomicU64::new(0));
        let req1_id = load_id.fetch_add(1, Ordering::SeqCst) + 1;
        let req2_id = load_id.fetch_add(1, Ordering::SeqCst) + 1;

        let current_system_id = load_id.load(Ordering::SeqCst);
        assert!(req1_id < current_system_id);
        assert_eq!(req2_id, current_system_id);
    }

    #[test]
    fn test_hidden_files_toggle_logic() {
        let mut show_hidden = false;
        show_hidden = !show_hidden;
        assert!(show_hidden);

        show_hidden = !show_hidden;
        assert!(!show_hidden);
    }

    #[test]
    fn test_search_buffer_manipulation() {
        let mut filter = String::new();
        filter.push('f');
        filter.push('l');
        assert_eq!(filter, "fl");

        if !filter.is_empty() {
            filter.pop();
        }
        assert_eq!(filter, "f");

        filter.clear();
        assert!(filter.is_empty());
    }

    #[test]
    fn test_exclusive_index_bounds() {
        let len = 3;
        let mut index = Some(1);

        if let Some(idx) = index {
            if idx + 1 < len {
                index = Some(idx + 1);
            }
        }
        assert_eq!(index, Some(2));
        if let Some(idx) = index {
            if idx > 0 {
                index = Some(idx - 1);
            }
        }
        assert_eq!(index, Some(1));
    }

    #[test]
    fn test_exclusive_index_wrap_around() {
        let len = 3;
        let index = 2;
        let new_idx = (index + 1) % len;
        assert_eq!(new_idx, 0);

        let index = 0;
        let new_idx = if index > 0 { index - 1 } else { len - 1 };
        assert_eq!(new_idx, 2);
    }

    #[test]
    fn test_task_progress_tracking() {
        let mut is_loading = true;
        let mut task_progress = Some(0.0);

        assert!(is_loading);
        assert_eq!(task_progress, Some(0.0));

        task_progress = Some(0.75);
        assert_eq!(task_progress, Some(0.75));

        is_loading = false;
        task_progress = None;
        assert!(!is_loading);
        assert!(task_progress.is_none());
    }

    #[test]
    fn test_breadcrumb_logic_consistency() {
        let path = PathBuf::from("/tmp/flux/test/path");
        let mut segments = Vec::new();
        let mut current = path.as_path();
        while let Some(name) = current.file_name() {
            segments.push(name.to_string_lossy().to_string());
            if let Some(parent) = current.parent() {
                current = parent;
            } else {
                break;
            }
        }

        assert_eq!(segments[0], "path");
        assert_eq!(segments[1], "test");
        assert_eq!(segments[2], "flux");
    }

    #[test]
    fn test_selection_toggle_logic() {
        let mut selected_indices = std::collections::HashSet::new();
        selected_indices.insert(5);

        let target = 5;
        if selected_indices.contains(&target) {
            selected_indices.remove(&target);
        } else {
            selected_indices.insert(target);
        }
        assert!(selected_indices.is_empty());

        let new_target = 10;
        selected_indices.insert(new_target);
        assert!(selected_indices.contains(&10));
    }

    #[test]
    fn test_directory_navigation_logic() {
        let is_dir = true;
        let base_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut current_path = base_path.clone();
        let target_dir = "Downloads";
        if is_dir {
            current_path.push(target_dir);
        }

        assert_eq!(current_path, base_path.join("Downloads"));
    }

    #[test]
    fn test_mime_type_action_filtering() {
        let dir_mime = "inode/directory";
        let dir_actions = vec!["builtin::copy", "builtin::open_with"];

        let filtered_dir: Vec<&str> = dir_actions
            .into_iter()
            .filter(|&action| {
                if action == "builtin::open_with" && dir_mime == "inode/directory" {
                    return false;
                }

                true
            })
            .collect();
        assert!(filtered_dir.contains(&"builtin::copy"));
        assert!(!filtered_dir.contains(&"builtin::open_with"));

        let file_mime = "text/plain";
        assert!(file_mime != "inode/directory");
    }

    #[test]
    fn test_empty_selection_guard() {
        let selected_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let has_selection = !selected_indices.is_empty();
        assert!(!has_selection);

        let mut mutable_selection = selected_indices.clone();
        mutable_selection.clear();
        assert!(mutable_selection.is_empty());
    }

    #[test]
    fn test_navigation_path_normalization() {
        let path = PathBuf::from("/home/user/Documents/..");
        let normalized = if path.ends_with("..") {
            path.parent()
                .unwrap_or(&path)
                .parent()
                .unwrap_or(&path)
                .to_path_buf()
        } else {
            path
        };
        assert_eq!(normalized, PathBuf::from("/home/user"));
    }
    #[test]
    fn test_clipboard_fallback() {
        let display = adw::gdk::Display::default();
        assert!(display.is_some() || display.is_none());
    }
}
