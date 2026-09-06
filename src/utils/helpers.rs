use crate::model::{AppMsg, FluxApp, PathSegment, SortBy};
use crate::ui::{constants, SidebarPlace};
use crate::utils;
use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use relm4::prelude::*;
use std::cell::RefCell;
use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

impl FluxApp {
    /// Returns true if the current path is inside an archive that supports full extraction.
    pub fn can_extract_current_archive(&self) -> bool {
        let path_str = self.current_path.to_string_lossy();
        if !path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
            return false;
        }
        crate::services::archive::parse_archive_uri(&path_str)
            .map(|(archive_path, _)| {
                let name = archive_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();

                // Exclude ISO images and single-file stream formats (.gz, .xz, etc.)
                name.ends_with(".zip")
                    || name.ends_with(".7z")
                    || name.ends_with(".rar")
                    || name.ends_with(".tar")
                    || name.ends_with(".tar.gz")
                    || name.ends_with(".tgz")
                    || name.ends_with(".tar.bz2")
                    || name.ends_with(".tbz2")
                    || name.ends_with(".tar.xz")
                    || name.ends_with(".txz")
                    || name.ends_with(".tar.zst")
                    || name.ends_with(".tzst")
                    || name.ends_with(".tar.lz4")
                    || name.ends_with(".deb")
            })
            .unwrap_or(false)
    }

    /// Returns the display-friendly string for the current sorting state.
    pub fn sort_status(&self) -> String {
        let arrow = if self.sort_ascending { " ↑" } else { " ↓" };
        format!(
            "{}{}",
            match self.sort_by {
                SortBy::Name => crate::i18n::tr("Name"),
                SortBy::Date => crate::i18n::tr("Date"),
                SortBy::Size => crate::i18n::tr("Size"),
                SortBy::Type => crate::i18n::tr("Type"),
            },
            arrow
        )
    }

    /// Returns the `GVariant` string state for the stateful `app.sort-direction` radio action.
    pub fn sort_direction_state(&self) -> gtk::glib::Variant {
        gtk::glib::Variant::from(if self.sort_ascending { "asc" } else { "desc" })
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
            if modifiers == gdk::ModifierType::CONTROL_MASK {
                sender.input(AppMsg::AddToSidebarPermanent);
            } else {
                sender.input(AppMsg::AddExclusive(None));
            }
            return glib::Propagation::Stop;
        }

        // 2. Focus Bypass Check (The Search Logic)
        let is_editable = ctrl
            .widget()
            .and_then(|w| w.root())
            .and_then(|r| r.focus())
            .map(|f| f.type_().is_a(gtk::Editable::static_type()))
            .unwrap_or(false);

        // If any text entry in the window is focused (rename entry, search, etc.),
        // do not steal Return, Escape, or typing!
        if is_editable {
            if (header_view == "search" || header_view == "entry") && keyval == gdk::Key::Escape {
                sender.input(AppMsg::CancelContentSearch);
                sender.input(AppMsg::UpdateFilter(String::new()));
                sender.input(AppMsg::SwitchHeader("path".to_string()));
                return glib::Propagation::Stop;
            }
            return glib::Propagation::Proceed;
        }

        // 3. Global Capture (Type-to-search)
        // Only triggers when no modifiers (like Ctrl or Shift) are held,
        // allowing modifiers to be used for native multi-selection expansion.
        // Allow unmodified keys (letters/numbers) AND Shift+key (for symbols)
        let forbidden = gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK;

        if !modifiers.intersects(forbidden) {
            if let Some(c) = keyval.to_unicode() {
                // Allow alphanumerics, tags (#), content search (:), and size filters (<)
                if c.is_ascii_alphanumeric() || c == ':' || c == '<' || c == '#' {
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

    /// Propagates the current `is_list_mode` flag to every `FileItem` in the grid.
    ///
    /// Must be called after toggling list mode so `bind()` re-renders each item
    /// with the correct orientation and icon size.
    pub fn sync_list_mode(&mut self) {
        let mode = self.is_list_mode;
        let size = if mode {
            self.current_list_icon_size
        } else {
            self.current_icon_size
        };
        for i in 0..self.files.len() {
            if let Some(wrapper) = self.files.get(i) {
                let mut item = wrapper.borrow().clone();
                // Update both mode and icon size if either changed
                if item.is_list_mode != mode || item.icon_size != size {
                    item.is_list_mode = mode;
                    item.icon_size = size;
                    self.files.remove(i);
                    self.files.insert(i, item);
                }
            }
        }
    }

    /// Rebuilds the sidebar navigation list from XDG directories, mounts, and configuration.
    pub fn refresh_sidebar(&mut self) {
        let mut guard = self.sidebar.guard();
        guard.clear();

        let recents_place = || crate::ui::SidebarPlace {
            name: crate::i18n::tr("Recents"),
            icon: "document-open-recent-symbolic".to_string(),
            path: std::path::PathBuf::from(crate::ui::constants::RECENT_URI),
            is_mount: false,
            is_section_label: false,
        };

        let get_xdg_name = |p: &PathBuf| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.to_string_lossy().to_string())
        };

        // 1. Core XDG Directories - Conditioned on show_xdg_dirs config
        if self.config.ui.show_xdg_dirs {
            if let Some(p) = dirs::home_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "user-home-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::desktop_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "user-desktop-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::download_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-download-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::document_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-documents-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::picture_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-pictures-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::video_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-videos-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
        }

        // 2. Custom Sidebar logic
        let show_recents = self.config.ui.show_recents;
        let recents_row = self.config.ui.recents_row;
        for (idx, custom) in self.config.sidebar.iter().enumerate() {
            if show_recents && idx == recents_row {
                guard.push_back(recents_place());
            }
            if custom.kind.as_deref() == Some("label") {
                guard.push_back(SidebarPlace {
                    name: custom.name.clone(),
                    icon: String::new(),
                    path: PathBuf::new(),
                    is_mount: false,
                    is_section_label: true,
                });
                continue;
            }

            let path = if custom.path.starts_with('~') {
                dirs::home_dir()
                    .map(|h| PathBuf::from(custom.path.replace('~', &h.to_string_lossy())))
                    .unwrap_or_else(|| PathBuf::from(&custom.path))
            } else {
                PathBuf::from(&custom.path)
            };

            let mut name = custom.name.clone();
            // Translate Trash if it's the default English name
            if custom.path == constants::TRASH_URI && name == "Trash" {
                name = crate::i18n::tr("Trash");
            } else if custom.path == "tags://" && name == "Tags" {
                name = crate::i18n::tr("Tags");
            }

            guard.push_back(SidebarPlace {
                name,
                icon: custom.icon.clone(),
                path,
                is_mount: false,
                is_section_label: false,
            });
        }

        if show_recents && recents_row >= self.config.sidebar.len() {
            guard.push_back(recents_place());
        }

        // 3. Mounts - Offloaded to background thread to prevent blocking the UI loop
        if let Some(sender) = crate::model::SENDER.get().cloned() {
            relm4::spawn_blocking(move || {
                let mounts = utils::get_system_mounts();
                let _ = sender.send(AppMsg::SystemMountsReady(mounts));
            });
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

        let path_str = self.current_path.to_string_lossy();

        // Handle virtual archive breadcrumbs differently so they show human-readable names
        if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
            if let Some((archive_path, inner)) =
                crate::services::archive::parse_archive_uri(&path_str)
            {
                let archive_name = archive_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Archive".to_string());

                let root_uri = crate::services::archive::build_archive_uri(&archive_path, "");
                guard.push_back(PathSegment {
                    name: archive_name,
                    path: root_uri,
                });

                if !inner.is_empty() {
                    let mut current_inner = PathBuf::new();
                    for component in Path::new(&inner).components() {
                        let name = component.as_os_str().to_string_lossy().to_string();
                        if name.is_empty() {
                            continue;
                        }
                        current_inner.push(&name);
                        let segment_uri = crate::services::archive::build_archive_uri(
                            &archive_path,
                            &current_inner.to_string_lossy(),
                        );
                        guard.push_back(PathSegment {
                            name,
                            path: segment_uri,
                        });
                    }
                }

                if let Some(entry) = self.header_path_entry.upgrade() {
                    entry.set_text(&path_str);
                    entry.set_position(entry.text_length() as i32);
                }
                return;
            }
        }

        // Network URIs (ftp://, smb://, sftp://, etc.) cannot be decomposed via
        // PathBuf::components - the stdlib has no URI awareness and will mangle
        // the scheme double-slash into bogus local paths. Split on '/' manually
        // and reconstruct each breadcrumb as a well-formed URI.
        if crate::services::network::is_network_uri(&self.current_path) {
            let uri = path_str.trim_end_matches('/');

            if let Some((scheme, after_scheme)) = uri.split_once("://") {
                let slash_pos = after_scheme.find('/');
                let authority = &after_scheme[..slash_pos.unwrap_or(after_scheme.len())];

                let root_uri = format!("{}://{}", scheme, authority);
                guard.push_back(PathSegment {
                    name: authority.to_string(),
                    path: PathBuf::from(&root_uri),
                });

                if let Some(path_part) = slash_pos.map(|p| &after_scheme[p + 1..]) {
                    let mut acc = root_uri.clone();
                    for segment in path_part.split('/').filter(|s| !s.is_empty()) {
                        acc.push('/');
                        acc.push_str(segment);
                        guard.push_back(PathSegment {
                            name: segment.to_string(),
                            path: PathBuf::from(&acc),
                        });
                    }
                }
            }

            if let Some(entry) = self.header_path_entry.upgrade() {
                entry.set_text(&path_str);
                entry.set_position(entry.text_length() as i32);
            }
            return;
        }

        let mut path_acc = PathBuf::from("/");
        let mut segments = Vec::new();

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

        let max_visible = constants::MAX_BREADCRUMBS;
        let skip = segments.len().saturating_sub(max_visible);

        for segment in segments.into_iter().skip(skip) {
            guard.push_back(segment);
        }

        if let Some(entry) = self.header_path_entry.upgrade() {
            entry.set_text(&path_str);
            entry.set_position(entry.text_length() as i32);
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

    /// Returns `(path, is_dir)` pairs for all currently selected items.
    ///
    /// Mirrors [`get_selection`] exactly but preserves the `is_dir` flag from
    /// the [`FileItem`] model. Required when the caller must distinguish virtual
    /// archive directories (whose paths are `archive://` URIs, never real
    /// filesystem paths) from regular files without issuing a syscall.
    pub(crate) fn get_selection_with_meta(&self) -> Vec<(PathBuf, bool)> {
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
        let mut result = Vec::new();

        if let Some((mut iter, first_idx)) = gtk::BitsetIter::init_first(&bitset) {
            let mut visual_indices = Vec::new();
            let mut current = Some(first_idx);
            while let Some(idx) = current {
                visual_indices.push(idx);
                current = iter.next();
            }

            if self.filter.is_empty() {
                for idx in visual_indices {
                    if let Some(wrapper) = self.files.get(idx) {
                        let item = wrapper.borrow();
                        result.push((item.path.clone(), item.is_dir));
                    }
                }
            } else {
                let query_lc = self.filter.to_lowercase();
                let mut match_count = 0u32;

                for i in 0..self.files.len() {
                    if let Some(wrapper) = self.files.get(i) {
                        let item = wrapper.borrow();
                        if item.name.to_lowercase().contains(&query_lc) {
                            if visual_indices.contains(&match_count) {
                                result.push((item.path.clone(), item.is_dir));
                            }
                            match_count += 1;
                        }
                    }
                }
            }
        }
        result
    }

    /// Returns the filesystem path of the first currently selected item.
    pub(crate) fn get_selected_path(&self) -> Option<PathBuf> {
        self.get_selection().into_iter().next()
    }

    /// Dispatches the correct action for each `(path, is_dir)` item pair.
    ///
    /// Centralises the routing logic shared by `AppMsg::Open` and `AppMsg::Activate`
    /// so that both code paths behave identically regardless of how the items were
    /// resolved (by grid position or by selection model).
    ///
    /// # Behaviour
    /// - `archive://` directory → `Navigate` into the virtual sub-directory.
    /// - `archive://` file → extract to a `NamedTempFile` and `xdg-open` it.
    /// - Real directory → `Navigate`.
    /// - Browsable archive on disk → `EnterArchive`.
    /// - Any other file → `open_file` (xdg-open).
    pub(crate) fn activate_items(
        &self,
        items: Vec<(PathBuf, bool)>,
        sender: &relm4::AsyncComponentSender<Self>,
    ) {
        #[allow(clippy::never_loop)]
        for (path, is_dir) in items {
            let path_str = path.to_string_lossy();

            // 1. Check if we are inside a virtual archive path (/archive://...)
            if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
                if let Some((archive_path, inner)) =
                    crate::services::archive::parse_archive_uri(&path_str)
                {
                    if is_dir {
                        // Navigating deeper into a virtual archive folder
                        sender.input(AppMsg::Navigate(path));
                    } else if crate::services::archive::is_browsable_archive(Path::new(&inner)) {
                        sender.input(AppMsg::EnterArchive(path));
                    } else {
                        // Extracting and launching a file from inside the archive
                        let sender_clone = sender.clone();
                        let cached_pwd = self.cached_archive_password.clone();

                        // Compute parent directory prefix for password re-prompting
                        let parent_prefix = Path::new(&inner)
                            .parent()
                            .and_then(|p| p.to_str())
                            .unwrap_or("")
                            .to_string();

                        relm4::spawn_blocking(move || {
                            match crate::services::archive::extract_entry_to_tempfile(
                                &archive_path,
                                &inner,
                                cached_pwd.as_deref(),
                            ) {
                                Ok(tmp) => {
                                    let tmp_path = tmp.path().to_path_buf();
                                    tmp.keep().ok();
                                    crate::utils::open_file(tmp_path);
                                }
                                Err(crate::services::archive::ArchiveError::PasswordRequired) => {
                                    sender_clone.input(AppMsg::PromptArchivePassword {
                                        archive_path,
                                        prefix: parent_prefix.clone(),
                                        wrong_password: false,
                                    });
                                }
                                Err(crate::services::archive::ArchiveError::WrongPassword) => {
                                    sender_clone.input(AppMsg::PromptArchivePassword {
                                        archive_path,
                                        prefix: parent_prefix,
                                        wrong_password: true,
                                    });
                                }
                                Err(e) => {
                                    sender_clone.input(AppMsg::ShowToast(e.to_string()));
                                }
                            }
                        });
                    }
                }
                break;
            }
            // 2. Regular filesystem directory navigation
            else if is_dir {
                sender.input(AppMsg::Navigate(path));
                break;
            }
            // 3. Opening a physical archive file from disk (.7z, .zip, etc.)
            else if crate::services::archive::is_browsable_archive(&path) {
                sender.input(AppMsg::EnterArchive(path));
                break;
            }
            // 3.5. LUKS encrypted image file or standard file opening offloaded to background worker
            else {
                let sender_clone = sender.clone();
                let path_clone = path.clone();
                relm4::spawn_blocking(move || {
                    if crate::services::luks::is_luks_image(&path_clone) {
                        sender_clone.input(AppMsg::UnlockLuksImage { path: path_clone });
                    } else {
                        crate::utils::open_file(path_clone);
                    }
                });
                break;
            }
        }
    }

    /// Moves a collection of source files/directories into a target destination directory.
    pub fn handle_move_files_to_target(
        &mut self,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let mut moved_any = false;
        for src in sources {
            if src == destination {
                continue;
            }
            if let Some(file_name) = src.file_name() {
                let dest_path = destination.join(file_name);
                if let Err(e) = std::fs::rename(&src, &dest_path) {
                    sender.input(AppMsg::ShowToast(format!("Failed to move file: {}", e)));
                } else {
                    moved_any = true;
                }
            }
        }
        if moved_any {
            sender.input(AppMsg::Refresh);
        }
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
            ("<Control><Shift>s", AppMsg::ToggleSortOrder),
            ("<Shift>s", AppMsg::CycleFolderPriority),
            ("<Control>c", AppMsg::Copy),
            ("<Control>x", AppMsg::Cut),
            ("<Control>v", AppMsg::Paste),
            ("Delete", AppMsg::Delete),
            ("<Control>l", AppMsg::PromptLocationDialog),
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

    /// Internal helper to populate the clipboard with the current selection.
    ///
    /// Args:
    ///     is_cut: If true, prefixes the text metadata with "cut" to signal a move operation.
    pub fn handle_clipboard_action(&self, is_cut: bool) {
        let selection = self.get_selection_with_meta();
        if selection.is_empty() {
            return;
        }

        let clipboard = gdk::Display::default().expect("No Display").clipboard();

        // 1. Build the standard URI list (text/uri-list).
        // For archive:// virtual paths, extract to a temp location first so that
        // external apps receive a real file:// URI they can act on.
        let mut uri_list = String::new();
        for (path, is_dir) in &selection {
            let path_str = path.to_string_lossy();
            let uri = if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
                if let Some((archive_path, inner)) =
                    crate::services::archive::parse_archive_uri(&path_str)
                {
                    let cached_pwd = self.cached_archive_password.as_deref();
                    if *is_dir {
                        match crate::services::archive::extract_dir_to_tempdir(
                            &archive_path,
                            &inner,
                            cached_pwd,
                        ) {
                            Ok(tmp_dir) => gio::File::for_path(&tmp_dir).uri().to_string(),
                            Err(_) => continue,
                        }
                    } else {
                        match crate::services::archive::extract_entry_to_tempfile(
                            &archive_path,
                            &inner,
                            cached_pwd,
                        ) {
                            Ok(tmp) => {
                                let tmp_path = tmp.path().to_path_buf();
                                tmp.keep().ok();
                                gio::File::for_path(&tmp_path).uri().to_string()
                            }
                            Err(_) => continue,
                        }
                    }
                } else {
                    continue;
                }
            } else {
                gio::File::for_path(path).uri().to_string()
            };

            uri_list.push_str(&uri);
            uri_list.push_str("\r\n");
        }

        if uri_list.is_empty() {
            return;
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

    /// Appends a batch of extension/glob search results to the grid.
    pub fn handle_extension_search_batch(
        &mut self,
        results: Vec<crate::services::extension_search::ExtensionMatch>,
        session: u64,
    ) {
        if self.load_id.load(Ordering::SeqCst) != session {
            return;
        }

        for item in results {
            let icon = utils::get_icon_for_path(&item.path, false);
            let meta = std::fs::metadata(&item.path).ok();
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let mtime = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            self.files.append(crate::ui::FileItem {
                name: item.display,
                icon,
                thumbnail: None,
                is_dir: false,
                path: item.path,
                icon_size: self.current_list_icon_size,
                size,
                mtime,
                is_editing: false,
                is_foreign_owner: false,
                expand_labels: false,
                is_list_mode: true,
                is_custom_icon: false,
                active_path: std::rc::Rc::new(std::cell::RefCell::new(None)),
                grid_idx: self.files.len(),
                max_width_chars: self.config.ui.max_width_chars,
                grid_spacing: self.config.ui.grid_spacing,
            });
        }
    }

    /// During a content search the directory monitor only watches current_path
    /// (flat, non-recursive), so FileDeleted never fires for files in
    /// subdirectories. Remove all grid entries for the selected paths immediately
    /// before handing off to the trash service.
    pub fn remove_search_results_for_paths(&mut self, paths: &[PathBuf]) {
        let mut i = 0;
        while i < self.files.len() {
            if self
                .files
                .get(i)
                .is_some_and(|r| paths.contains(&r.borrow().path))
            {
                self.files.remove(i);
            } else {
                i += 1;
            }
        }
    }
}

thread_local! {
    static ACTIVE_CSS_PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

/// Applies the active theme CSS to the global GTK display, cleanly removing the old stylesheet.
pub fn load_custom_css() {
    let config = crate::utils::load_config();
    let config_dir = dirs::config_dir().unwrap_or_default().join("flux");

    let mut css_data = None;

    if let Some(ref theme_name) = config.ui.theme {
        if theme_name != "default" {
            let theme_filename = format!("{}.css", theme_name);

            let local_theme = dirs::data_local_dir()
                .unwrap_or_default()
                .join("flux/themes")
                .join(&theme_filename);

            let system_theme = PathBuf::from("/usr/share/flux/themes").join(&theme_filename);
            let user_conf_theme = config_dir.join("themes").join(&theme_filename);

            css_data = fs::read_to_string(&local_theme)
                .or_else(|_| fs::read_to_string(&user_conf_theme))
                .or_else(|_| fs::read_to_string(&system_theme))
                .ok();
        }
    }

    if css_data.is_none() {
        css_data = fs::read_to_string(config_dir.join("style.css")).ok();
    }

    if let Some(display) = adw::gdk::Display::default() {
        ACTIVE_CSS_PROVIDER.with(|cell| {
            let mut guard = cell.borrow_mut();

            if let Some(ref old_provider) = *guard {
                gtk::style_context_remove_provider_for_display(&display, old_provider);
            }

            let new_provider = gtk::CssProvider::new();

            if let Some(ref data) = css_data {
                new_provider.load_from_data(data);
            } else {
                new_provider.load_from_data("");
            }

            gtk::style_context_add_provider_for_display(
                &display,
                &new_provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );

            *guard = Some(new_provider);
        });
    }

    if let Some(ref theme_name) = config.ui.theme {
        let style_manager = adw::StyleManager::default();
        if theme_name.contains("dark") {
            style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
        } else if theme_name.contains("light") {
            style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
        } else {
            style_manager.set_color_scheme(adw::ColorScheme::Default);
        }
    }
}

/// Computes the right-side status string containing active filters and free volume space.
pub fn format_right_status(current_path: &Path, extension_filter: Option<&[String]>) -> String {
    let mut parts = Vec::new();

    if let Some(patterns) = extension_filter {
        if !patterns.is_empty() {
            parts.push(format!("[filter: {}]", patterns.join(", ")));
        }
    }

    if current_path.is_absolute() && current_path.exists() {
        if let Ok(c_path) = CString::new(current_path.as_os_str().as_bytes()) {
            let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
            if unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) } == 0 {
                let stat = unsafe { stat.assume_init() };
                let free_bytes = (stat.f_bsize) * (stat.f_bavail);
                parts.push(format!("{} free", gtk::glib::format_size(free_bytes)));
            }
        }
    }

    parts.join(" · ")
}

/// Spawns a new application instance rooted at `path`.
///
/// Uses the running executable path rather than a hardcoded binary name so
/// dev builds (`flux-fm`) and installed builds (`flux`) both work correctly.
///
/// Returns `false` if the path is not a directory or the exe path cannot be
/// resolved, `true` if the child process was spawned successfully.
pub fn open_new_instance(path: &std::path::Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    std::process::Command::new(exe).arg(path).spawn().is_ok()
}

/// Checks if a paste operation is recursive (pasting a folder into itself or a subfolder).
#[allow(dead_code)]
pub fn is_recursive_paste(src: &Path, dest_dir: &Path) -> bool {
    dest_dir.starts_with(src)
}
