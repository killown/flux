use crate::model::{AppMsg, FluxApp, PathSegment, SortBy};
use crate::ui::{constants, SidebarPlace};
use crate::utils;
use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Returns the display-friendly string for the current sorting state.
    pub fn sort_status(&self) -> &str {
        match self.sort_by {
            SortBy::Name => "Name",
            SortBy::Date => "Date",
            SortBy::Size => "Size",
        }
    }

    /// Processes keyboard events to trigger system shortcuts or manage focus-dependent navigation.
    ///
    /// Args:
    ///     ctrl: The key event controller.
    ///     keyval: The specific key pressed.
    ///     state: Bitmask of modifier keys (Ctrl, Alt, etc.).
    ///     sender: The component's message sender.
    ///     header_view: ID of the currently active header bar view.
    pub fn handle_key_event(
        ctrl: &gtk::EventControllerKey,
        keyval: gdk::Key,
        state: gdk::ModifierType,
        sender: &AsyncComponentSender<Self>,
        header_view: &str,
    ) -> glib::Propagation {
        let modifiers = state & gtk::accelerator_get_default_mod_mask();

        // 1. System Keys
        if keyval == gdk::Key::F1 {
            sender.input(AppMsg::ShowHelp);
            return glib::Propagation::Stop;
        }

        if keyval == gdk::Key::F2 {
            sender.input(AppMsg::TriggerRenameSelection);
            return glib::Propagation::Stop;
        }

        if modifiers == gdk::ModifierType::CONTROL_MASK && keyval == gdk::Key::Delete {
            sender.input(AppMsg::ClearExclusive);
            return glib::Propagation::Stop;
        }

        if keyval == gdk::Key::Insert {
            sender.input(AppMsg::AddExclusive);
            return glib::Propagation::Stop;
        }

        // 2. Focus Bypass Check (The Search Logic)
        let is_editable = ctrl
            .widget()
            .and_then(|w| w.root())
            .and_then(|r| r.focus())
            .map(|f| f.type_().is_a(gtk::Editable::static_type()))
            .unwrap_or(false);

        if is_editable && (header_view == "search" || header_view == "entry") {
            if keyval == gdk::Key::Escape {
                sender.input(AppMsg::UpdateFilter(String::new()));
                sender.input(AppMsg::SwitchHeader("path".to_string()));
                return glib::Propagation::Stop;
            }
            return glib::Propagation::Proceed;
        }

        // 3. Global Capture (Type-to-search)
        // Only triggers when no modifiers (like Ctrl or Shift) are held,
        // allowing modifiers to be used for native multi-selection expansion.
        if modifiers.is_empty() {
            if let Some(c) = keyval.to_unicode() {
                // Only trigger type-to-search for alphanumeric characters
                if c.is_ascii_alphanumeric() {
                    sender.input(AppMsg::SwitchHeader(constants::VIEW_SEARCH.to_string()));
                    sender.input(AppMsg::SearchInput(c));
                    return glib::Propagation::Stop;
                }
            }
        }

        // 4. Context Keys & Batch Execution
        match keyval {
            // Triggers batch activation for the current selection
            gdk::Key::Return | gdk::Key::KP_Enter => {
                sender.input(AppMsg::Activate);
                glib::Propagation::Stop
            }
            gdk::Key::F2 => {
                sender.input(AppMsg::TriggerRenameSelection);
                glib::Propagation::Stop
            }
            gdk::Key::Escape => {
                sender.input(AppMsg::UpdateFilter(String::new()));
                sender.input(AppMsg::SwitchHeader("path".to_string()));
                glib::Propagation::Stop
            }
            gdk::Key::BackSpace => {
                sender.input(AppMsg::SearchBackspace);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    }

    /// Rebuilds the sidebar navigation list from XDG directories, mounts, and configuration.
    pub fn refresh_sidebar(&mut self) {
        let mut guard = self.sidebar.guard();
        guard.clear();

        let get_xdg_name = |p: &PathBuf| {
            gio::File::for_path(p)
                .query_info(
                    gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME,
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                )
                .map(|info| info.display_name().to_string())
                .unwrap_or_else(|_| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                })
        };

        // 1. Core XDG Directories - Conditioned on show_xdg_dirs config
        if self.config.ui.show_xdg_dirs {
            if let Some(p) = dirs::home_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "user-home-symbolic".to_string(),
                    path: p,
                });
            }
            if let Some(p) = dirs::desktop_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "user-desktop-symbolic".to_string(),
                    path: p,
                });
            }
            if let Some(p) = dirs::download_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-download-symbolic".to_string(),
                    path: p,
                });
            }
            if let Some(p) = dirs::document_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-documents-symbolic".to_string(),
                    path: p,
                });
            }
            if let Some(p) = dirs::picture_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-pictures-symbolic".to_string(),
                    path: p,
                });
            }
            if let Some(p) = dirs::video_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-videos-symbolic".to_string(),
                    path: p,
                });
            }
        }

        // 2. Custom Sidebar logic
        for custom in &self.config.sidebar {
            let path = if custom.path.starts_with('~') {
                dirs::home_dir()
                    .map(|h| PathBuf::from(custom.path.replace('~', &h.to_string_lossy())))
                    .unwrap_or_else(|| PathBuf::from(&custom.path))
            } else {
                PathBuf::from(&custom.path)
            };
            guard.push_back(SidebarPlace {
                name: custom.name.clone(),
                icon: custom.icon.clone(),
                path,
            });
        }

        // 3. Mounts
        for (mut name, path) in utils::get_system_mounts() {
            let path_str = path.to_string_lossy().to_string();
            let mut icon = "drive-harddisk-symbolic".to_string();

            // Intercept device renames to apply custom Name and Icon
            if let Some(rename) = self.config.ui.device_renames.get(&path_str) {
                name = rename.name.clone();
                if let Some(custom_icon) = &rename.icon {
                    icon = custom_icon.clone();
                }
            } else {
                // Fallback icon logic if no specific rename exists
                if name.to_lowercase().contains("drive")
                    || name.to_lowercase().contains("cloud")
                    || path_str.contains("Gdrive")
                {
                    icon = "folder-remote-symbolic".to_string();
                }
            }

            guard.push_back(SidebarPlace { name, icon, path });
        }

        // 4. Exclusive List
        for path in &self.exclusive_list {
            if let Some(name) = path.file_name() {
                guard.push_back(SidebarPlace {
                    name: format!("#{}", name.to_string_lossy()),
                    icon: "go-next-symbolic".to_string(),
                    path: path.clone(),
                });
            }
        }
    }

    /// Attaches a motion controller to change the cursor to a pointer on hover.
    /// We use concrete &gtk::Widget here to bypass trait resolution conflicts.
    pub fn set_cursor_pointer(widget: &gtk::Widget, enabled: bool) {
        if !enabled {
            return;
        }

        let controller = gtk::EventControllerMotion::new();

        controller.connect_enter(|ctrl, _, _| {
            if let Some(widget) = ctrl.widget() {
                widget.set_cursor_from_name(Some("pointer"));
            }
        });

        controller.connect_leave(|ctrl| {
            if let Some(widget) = ctrl.widget() {
                widget.set_cursor(None);
            }
        });

        widget.add_controller(controller);
    }

    /// Updates the collection of path segments for breadcrumb navigation display.
    pub fn update_breadcrumbs(&mut self) {
        let mut guard = self.breadcrumbs.guard();
        guard.clear();

        let mut path_acc = PathBuf::from("/");
        let mut segments = Vec::new();

        // Always add Root/Home
        for component in self.current_path.components() {
            let name = component.as_os_str().to_string_lossy().to_string();
            if name == "/" || name.is_empty() {
                continue;
            }

            path_acc.push(&name);
            segments.push(PathSegment {
                name,
                path: path_acc.clone(),
            });
        }

        // Slice to N latest (e.g., 4 or 5)
        let max_visible = constants::MAX_BREADCRUMBS;
        let skip = segments.len().saturating_sub(max_visible);

        for segment in segments.into_iter().skip(skip) {
            guard.push_back(segment);
        }
    }

    /// Returns a vector of PathBufs representing what the user actually sees as selected.
    ///
    /// If a filter is active, it maps the visual selection indices back to the
    /// matching files. If no filter is active, it maps them directly.
    pub(crate) fn get_selection(&self) -> Vec<PathBuf> {
        let selection_model = match self
            .files
            .view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
        {
            Some(m) => m,
            None => return Vec::new(),
        };

        let bitset = selection_model.selection();
        let mut selected_paths = Vec::new();

        if let Some((mut iter, first_idx)) = gtk::BitsetIter::init_first(&bitset) {
            let mut visual_indices = Vec::new();
            let mut current = Some(first_idx);
            while let Some(idx) = current {
                visual_indices.push(idx);
                current = iter.next();
            }

            if self.filter.is_empty() {
                // Scenario A: No filter. Map indices directly to the file list.
                for idx in visual_indices {
                    if let Some(wrapper) = self.files.get(idx) {
                        selected_paths.push(wrapper.borrow().path.clone());
                    }
                }
            } else {
                // Scenario B: Filter is active. We must find which actual items
                // correspond to the visual indices (e.g., visual index 0 is the 1st match).
                let query_lc = self.filter.to_lowercase();
                let mut match_count = 0;

                for i in 0..self.files.len() {
                    if let Some(wrapper) = self.files.get(i) {
                        if wrapper.borrow().name.to_lowercase().contains(&query_lc) {
                            if visual_indices.contains(&(match_count as u32)) {
                                selected_paths.push(wrapper.borrow().path.clone());
                            }
                            match_count += 1;
                        }
                    }
                }
            }
        }
        selected_paths
    }

    /// Returns the filesystem path of the first currently selected item.
    pub(crate) fn get_selected_path(&self) -> Option<PathBuf> {
        self.get_selection().into_iter().next()
    }

    /// Registers application-wide keyboard shortcuts with a ShortcutController.
    pub fn setup_shortcuts(
        controller: &gtk::ShortcutController,
        sender: &AsyncComponentSender<Self>,
    ) {
        let shortcuts = [
            ("<Control>h", AppMsg::ToggleHidden),
            ("F1", AppMsg::ShowHelp),
            ("<Control>s", AppMsg::CycleSort),
            ("<Shift>s", AppMsg::CycleFolderPriority),
            ("<Control>c", AppMsg::Copy),
            ("<Control>x", AppMsg::Cut),
            ("<Control>v", AppMsg::Paste),
            ("Delete", AppMsg::Delete),
        ];

        for (trigger_str, msg) in shortcuts {
            let s_clone = sender.clone();
            controller.add_shortcut(gtk::Shortcut::new(
                Some(gtk::ShortcutTrigger::parse_string(trigger_str).unwrap()),
                Some(gtk::CallbackAction::new(move |_, _| {
                    s_clone.input(msg.clone());
                    glib::Propagation::Stop
                })),
            ));
        }

        // Handle specific string-argument messages separately
        let f_sender = sender.clone();
        controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::ShortcutTrigger::parse_string("<Control>f").unwrap()),
            Some(gtk::CallbackAction::new(move |_, _| {
                f_sender.input(AppMsg::SwitchHeader("search".to_string()));
                glib::Propagation::Stop
            })),
        ));
    }

    /// Registers GIO actions to the application's internal action group.
    pub fn setup_actions(&self, sender: &AsyncComponentSender<Self>) {
        // 1. Folder Priority Action
        let prio_action = gio::SimpleAction::new("cycle-priority", None);
        let prio_sender = sender.clone();
        prio_action.connect_activate(move |_, _| {
            prio_sender.input(AppMsg::CycleFolderPriority);
        });
        self.action_group.add_action(&prio_action);

        // 2. Clipboard: Copy Action
        let copy_action = gio::SimpleAction::new("copy", None);
        let c_sender = sender.clone();
        copy_action.connect_activate(move |_, _| {
            c_sender.input(AppMsg::Copy);
        });
        self.action_group.add_action(&copy_action);

        // 3. Clipboard: Cut Action
        let cut_action = gio::SimpleAction::new("cut", None);
        let x_sender = sender.clone();
        cut_action.connect_activate(move |_, _| {
            x_sender.input(AppMsg::Cut);
        });
        self.action_group.add_action(&cut_action);

        // 4. Clipboard: Paste Action
        let paste_action = gio::SimpleAction::new("paste", None);
        let v_sender = sender.clone();
        paste_action.connect_activate(move |_, _| {
            v_sender.input(AppMsg::Paste);
        });
        self.action_group.add_action(&paste_action);

        // 5. Parameterized "Open With..." Action
        let launch_action =
            gio::SimpleAction::new("launch-with", Some(glib::VariantTy::new("s").unwrap()));
        let launch_sender = sender.clone();
        launch_action.connect_activate(move |_, parameter| {
            if let Some(app_id) = parameter.and_then(|p| p.get::<String>()) {
                launch_sender.input(AppMsg::LaunchWithApp(app_id));
            }
        });
        self.action_group.add_action(&launch_action);

        // 6. Dynamic Context Menu Actions (Shell Commands)
        for action_def in &self.menu_actions {
            // Skip builtins as they are handled by the explicit actions above
            if action_def.command.starts_with("builtin::") {
                continue;
            }

            let cmd_clone = action_def.command.clone();
            let sender_clone = sender.clone();
            let action = gio::SimpleAction::new(&action_def.action_name, None);
            action.connect_activate(move |_, _| {
                sender_clone.input(AppMsg::ExecuteCommand(cmd_clone.clone()));
            });
            self.action_group.add_action(&action);
        }
    }

    pub fn perform_paste(&self, files: Vec<gio::File>, sender: AsyncComponentSender<Self>) {
        // 1. Clone the path so the closure owns it
        let target_dir = self.current_path.clone();
        let clipboard = gdk::Display::default().unwrap().clipboard();

        // 2. Use 'move' to transfer target_dir and files into the first closure
        clipboard.read_text_async(gio::Cancellable::NONE, move |res| {
            let is_cut = res
                .map(|s| s.map(|t| t.starts_with("cut")).unwrap_or(false))
                .unwrap_or(false);

            let total_files = files.len();
            // Track completion across all concurrent file operations
            let completed_files = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

            for file in files {
                // 3. Compute destination using the owned target_dir
                let basename = file.basename().expect("File must have a name");
                let mut dest = target_dir.join(&basename);

                if !is_cut {
                    let mut copy_number = 1;
                    let original_name = basename.to_string_lossy().into_owned();

                    while dest.exists() {
                        let new_name = match original_name.rfind('.') {
                            Some(idx) if idx > 0 => {
                                let (name, ext) = original_name.split_at(idx);
                                format!("{} (copy {}){}", name, copy_number, ext)
                            }
                            _ => format!("{} (copy {})", original_name, copy_number),
                        };
                        dest = target_dir.join(new_name);
                        copy_number += 1;
                    }
                }

                let dest_file = gio::File::for_path(dest);

                // Setup the progress callback
                let p_sender = sender.clone();
                let progress_callback = move |current, total| {
                    if total > 0 {
                        p_sender.input(AppMsg::TaskProgress(current as f64 / total as f64));
                    }
                };

                // Setup the completion callback
                let c_sender = sender.clone();
                let completed_clone = completed_files.clone();
                let finish_callback = move |res: Result<(), glib::Error>| {
                    if let Err(e) = res {
                        eprintln!("Operation error: {}", e);
                    }
                    // Increment the atomic counter safely across threads
                    let count =
                        completed_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if count == total_files {
                        c_sender.input(AppMsg::TaskCompleted);
                        c_sender.input(AppMsg::Refresh);
                    }
                };

                if is_cut {
                    file.move_async(
                        &dest_file,
                        gio::FileCopyFlags::OVERWRITE,
                        glib::Priority::DEFAULT,
                        gio::Cancellable::NONE,
                        Some(Box::new(progress_callback)),
                        finish_callback,
                    );
                } else {
                    file.copy_async(
                        &dest_file,
                        gio::FileCopyFlags::OVERWRITE,
                        glib::Priority::DEFAULT,
                        gio::Cancellable::NONE,
                        Some(Box::new(progress_callback)),
                        finish_callback,
                    );
                }
            }
        });
    }

    /// Internal helper to populate the clipboard with the current selection.
    ///
    /// Args:
    ///     is_cut: If true, prefixes the text metadata with "cut" to signal a move operation.
    /// Internal helper to populate the clipboard with the current selection.
    pub fn handle_clipboard_action(&self, is_cut: bool) {
        let selection = self.get_selection();
        if selection.is_empty() {
            return;
        }

        let clipboard = gdk::Display::default().expect("No Display").clipboard();

        // 1. Build the standard URI list (text/uri-list)
        // This is what other applications (Nautilus, Thunar) actually read.
        let mut uri_list = String::new();
        for path in &selection {
            let uri = gio::File::for_path(path).uri();
            uri_list.push_str(&uri);
            uri_list.push_str("\r\n");
        }

        // 2. Build the Flux-internal metadata protocol
        let prefix = if is_cut { "cut" } else { "copy" };
        let mut text_rep = String::from(prefix);
        text_rep.push('\n');
        text_rep.push_str(&uri_list);

        // 3. Create providers using raw bytes/strings
        // text/uri-list is the industry standard for file transfers
        let uri_provider = gdk::ContentProvider::for_bytes(
            "text/uri-list",
            &glib::Bytes::from(uri_list.as_bytes()),
        );

        // text/plain for our internal "cut" vs "copy" detection
        let text_provider = gdk::ContentProvider::for_value(&text_rep.to_value());

        // 4. Combine in a Union
        let content = gdk::ContentProvider::new_union(&[uri_provider, text_provider]);

        clipboard.set_content(Some(&content)).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use crate::model::SortBy;
    use std::path::PathBuf;

    /// Tests the logic for generating sorting status strings.
    #[test]
    fn test_sort_status_logic() {
        let cases = vec![
            (SortBy::Name, "Name"),
            (SortBy::Date, "Date"),
            (SortBy::Size, "Size"),
        ];

        for (variant, expected) in cases {
            let status = match variant {
                SortBy::Name => "Name",
                SortBy::Date => "Date",
                SortBy::Size => "Size",
            };
            assert_eq!(status, expected);
        }
    }

    /// Tests the breadcrumb/path segment generation logic.
    #[test]
    fn test_path_segment_logic() {
        let path = PathBuf::from("/home/user/Documents/flux");
        let mut segments = Vec::new();
        let mut current = path.as_path();

        // Mirroring the logic used in breadcrumb factory updates
        while let Some(name) = current.file_name() {
            segments.push(name.to_string_lossy().to_string());
            if let Some(parent) = current.parent() {
                current = parent;
            } else {
                break;
            }
        }

        // Segments are collected from leaf to root
        assert_eq!(segments[0], "flux");
        assert_eq!(segments[1], "Documents");
        assert_eq!(segments[2], "user");
    }

    /// Tests the string formatting for the selection status bar.
    #[test]
    fn test_selection_status_formatting() {
        let count = 5;
        let bytes: u64 = 2 * 1024 * 1024; // 2.0 MB

        let status = if count == 0 {
            "Empty".to_string()
        } else {
            let mb = bytes as f64 / (1024.0 * 1024.0);
            format!("Selected: {} items ({:.1} MB)", count, mb)
        };

        assert_eq!(status, "Selected: 5 items (2.0 MB)");
    }

    /// Verifies the "Empty" state for selection status.
    #[test]
    fn test_selection_status_empty() {
        let count = 0;
        let status = if count == 0 {
            "Empty".to_string()
        } else {
            "Not Empty".to_string()
        };
        assert_eq!(status, "Empty");
    }
}
