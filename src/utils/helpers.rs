use crate::model::{AppMsg, FluxApp, PathSegment, SortBy};
use crate::ui::{constants, SidebarPlace};
use crate::utils;
use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use relm4::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

// Global operation ID counter, monotonically increasing, unique per session.
pub(crate) static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

impl FluxApp {
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

    /// Constructs and presents the About window for the Flux file manager.
    ///
    /// Uses [`gtk::AboutDialog`] to display application metadata, version, and
    /// a clickable link to the upstream repository. Parented to the active window
    /// so it inherits the correct transient relationship and stays on top.
    pub fn show_about_window() {
        let about = gtk::AboutDialog::builder()
            .program_name("Flux")
            .version(env!("CARGO_PKG_VERSION"))
            .logo_icon_name("system-file-manager")
            .authors(vec!["killown".to_string()])
            .website("https://github.com/killown/flux")
            .website_label(crate::i18n::tr("Source Code"))
            .comments(crate::i18n::tr(
                "A fast, keyboard-driven file manager built with GTK4 and Libadwaita.",
            ))
            .license_type(gtk::License::Gpl30Only)
            .modal(true)
            .resizable(false)
            .build();

        if let Some(window) = gtk::Application::default().active_window() {
            about.set_transient_for(Some(&window));
        }

        about.present();
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

        if is_editable && (header_view == "search" || header_view == "entry") {
            if keyval == gdk::Key::Escape {
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
                // Only capture alphanumerics OR our chosen trigger characters
                if c.is_ascii_alphanumeric() || c == ':' || c == '<' {
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
        for i in 0..self.files.len() {
            if let Some(wrapper) = self.files.get(i) {
                let mut item = wrapper.borrow().clone();
                if item.is_list_mode != mode {
                    item.is_list_mode = mode;
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

            guard.push_back(SidebarPlace {
                name,
                icon,
                path,
                is_mount: true,
                is_section_label: false,
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
                return;
            }
        }

        // Network URIs (ftp://, smb://, sftp://, etc.) cannot be decomposed via
        // PathBuf::components - the stdlib has no URI awareness and will mangle
        // the scheme double-slash into bogus local paths. Split on '/' manually
        // and reconstruct each breadcrumb as a well-formed URI.
        if crate::services::network::is_network_uri(&self.current_path) {
            // Locate the scheme+authority prefix ("ftp://host:port", "smb://host", …)
            // Everything after that is the path portion to segment.
            let uri = path_str.trim_end_matches('/');

            if let Some((scheme, after_scheme)) = uri.split_once("://") {
                // after_scheme = "host:port/dir/subdir"
                let slash_pos = after_scheme.find('/');
                let authority = &after_scheme[..slash_pos.unwrap_or(after_scheme.len())];

                // Root breadcrumb: just scheme://authority
                let root_uri = format!("{}://{}", scheme, authority);
                guard.push_back(PathSegment {
                    name: authority.to_string(),
                    path: PathBuf::from(&root_uri),
                });

                // Remaining path segments, each building on the previous
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
            // 3.5. LUKS encrypted image file - probe magic bytes, prompt passphrase
            else if crate::services::luks::is_luks_image(&path) {
                sender.input(AppMsg::UnlockLuksImage { path });
                break;
            }
            // 4. Regular file opening via xdg-open
            else {
                crate::utils::open_file(path);
            }
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

    /// Executes paste operations unconditionally after the user confirms directory replacement.
    ///
    /// For directory sources, falls back to a blocking recursive copy/move on a thread-pool
    /// thread via `relm4::spawn_blocking` so the GTK main loop is never stalled. Regular
    /// files continue to use the GIO async copy path with progress callbacks.
    ///
    /// # Arguments
    ///
    /// * `files`   - The GIO file list to paste.
    /// * `is_cut`  - Indicator if the operation is a cut or a copy.
    /// * `forced`  - When `true`, directories are copied recursively via `copy_dir_recursive`
    ///   instead of `gio::File::copy_async` (which cannot handle directories).
    /// * `sender`  - Component sender for dispatching progress and completion messages.
    pub fn perform_paste_inner(
        &self,
        files: Vec<gio::File>,
        is_cut: bool,
        forced: bool,
        sender: AsyncComponentSender<Self>,
    ) {
        let target_dir = self.current_path.clone();

        if forced {
            let total_files = files.len();
            let completed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

            // Pre-scan total bytes so the dialog denominator is correct from the start.
            let total_bytes: u64 = files
                .iter()
                .filter_map(|f| f.path())
                .map(|p| crate::utils::helpers::scan_total_bytes(&p))
                .sum();

            for file in files {
                let src = match file.path().or_else(|| {
                    let uri = file.uri().to_string();
                    let clean_uri = uri.trim_end_matches('/');
                    gio::File::for_uri(clean_uri).path()
                }) {
                    Some(p) => p,
                    None => continue,
                };

                let orig_basename = match src.file_name() {
                    Some(f) => f.to_string_lossy().to_string(),
                    None => continue,
                };

                // Clean temporary extraction filenames (.tmpXyZ.favicon.svg -> favicon.svg)
                let clean_basename = if orig_basename.starts_with(".tmp") {
                    orig_basename
                        .split_once('.')
                        .and_then(|(_, rest)| rest.split_once('.'))
                        .map(|(_, real)| real.to_string())
                        .unwrap_or(orig_basename)
                } else {
                    orig_basename
                };

                let dest = target_dir.join(&clean_basename);
                let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
                let cancellable = gio::Cancellable::new();

                sender.input(AppMsg::TaskProgress {
                    id: task_id,
                    label: clean_basename.clone(),
                    current: 0,
                    total: total_bytes.max(1),
                    total_items: total_files,
                    cancellable: cancellable.clone(),
                });

                // Show dialog immediately for large / multi-file operations.
                if total_files >= 5 || total_bytes >= 32 * 1_024 * 1_024 {
                    sender.input(AppMsg::ShowTransferDialog);
                }

                let s = sender.clone();
                let completed_clone = completed.clone();
                let cancel = cancellable.clone();
                let task_id_delay = task_id;
                {
                    let s_delay = s.clone();
                    relm4::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        s_delay.input(AppMsg::ShowTransferDialogIfActive(task_id_delay));
                    });
                }

                relm4::spawn_blocking(move || {
                    let result = if is_cut {
                        gio::File::for_path(&src)
                            .move_(
                                &gio::File::for_path(&dest),
                                gio::FileCopyFlags::OVERWRITE
                                    | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                                Some(&cancel),
                                None,
                            )
                            .map_err(|e| e.to_string())
                    } else if src.is_dir() {
                        crate::utils::helpers::copy_dir_recursive_progress(
                            &src,
                            &dest,
                            &cancel,
                            task_id,
                            s.input_sender(),
                        )
                        .map_err(|e| e.to_string())
                    } else {
                        crate::utils::helpers::copy_file_progress(
                            &src,
                            &dest,
                            &cancel,
                            task_id,
                            s.input_sender(),
                        )
                        .map_err(|e| e.to_string())
                    };

                    if let Err(e) = result {
                        if !e.contains("cancelled") && !e.contains("Cancelled") {
                            let template = crate::i18n::tr("Copy failed: {}");
                            let msg = template.replace("{}", &e);
                            s.input(AppMsg::ShowToast(msg));
                        }
                    }

                    s.input(AppMsg::TaskCompleted(task_id));

                    let count =
                        completed_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if count == total_files {
                        s.input(AppMsg::Refresh);
                    }
                });
            }
        } else {
            Self::dispatch_paste_ops(files, is_cut, target_dir, sender);
        }
    }

    pub fn perform_paste(
        &self,
        files: Vec<gio::File>,
        is_cut: bool,
        sender: AsyncComponentSender<Self>,
    ) {
        self.perform_paste_inner(files, is_cut, false, sender);
    }

    /// Shared dispatch logic for non-conflicting paste operations.
    ///
    /// Handles both copy and cut (move) using GIO async APIs with per-byte
    /// progress callbacks feeding the task queue. Only called for paths where
    /// no directory conflict exists, conflicting directories go through
    /// `perform_paste_inner(forced=true)`.
    pub fn dispatch_paste_ops(
        files: Vec<gio::File>,
        is_cut: bool,
        target_dir: PathBuf,
        sender: AsyncComponentSender<Self>,
    ) {
        let mut dir_conflicts = Vec::new();

        let resolved_files: Vec<(PathBuf, String, bool)> = files
            .into_iter()
            .filter_map(|file| {
                let src_path = file.path().or_else(|| {
                    let uri = file.uri().to_string();
                    let clean_uri = uri.trim_end_matches('/');
                    gio::File::for_uri(clean_uri).path()
                })?;

                let orig_name = src_path.file_name()?.to_string_lossy().to_string();
                let clean_name = if orig_name.starts_with(".tmp") {
                    orig_name
                        .split_once('.')
                        .and_then(|(_, rest)| rest.split_once('.'))
                        .map(|(_, real)| real.to_string())
                        .unwrap_or(orig_name)
                } else {
                    orig_name
                };

                let is_dir = src_path.is_dir();
                Some((src_path, clean_name, is_dir))
            })
            .collect();

        for (_, name, is_dir) in &resolved_files {
            if *is_dir {
                let dest = target_dir.join(name);
                if dest.exists() && dest.is_dir() {
                    dir_conflicts.push(name.clone());
                }
            }
        }

        if !dir_conflicts.is_empty() {
            let gfiles = resolved_files
                .iter()
                .map(|(p, _, _)| gio::File::for_path(p))
                .collect();

            sender.input(AppMsg::ConfirmReplacePaste {
                files: gfiles,
                conflicts: dir_conflicts,
                is_cut,
            });
            return;
        }

        let total_files = resolved_files.len();

        // Pre-scan total bytes before spawning any I/O.
        let total_bytes: u64 = resolved_files
            .iter()
            .map(|(p, _, _)| crate::utils::helpers::scan_total_bytes(p))
            .sum();

        let completed_files = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        for (src_path, clean_name, is_dir) in resolved_files {
            let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
            let cancellable = gio::Cancellable::new();

            let mut dest = target_dir.join(&clean_name);

            if !is_cut && !is_dir {
                let mut copy_number = 1;
                let original_name = clean_name.clone();

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

            sender.input(AppMsg::TaskProgress {
                id: task_id,
                label: clean_name.clone(),
                current: 0,
                total: total_bytes.max(1),
                total_items: total_files,
                cancellable: cancellable.clone(),
            });

            // Immediate threshold check.
            if total_files >= 5 || total_bytes >= 32 * 1_024 * 1_024 {
                sender.input(AppMsg::ShowTransferDialog);
            }

            let s = sender.clone();
            let completed_clone = completed_files.clone();
            let cancel = cancellable.clone();
            let task_id_delay = task_id;
            {
                let s_delay = s.clone();
                relm4::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    s_delay.input(AppMsg::ShowTransferDialogIfActive(task_id_delay));
                });
            }

            relm4::spawn_blocking(move || {
                let result = if is_cut {
                    gio::File::for_path(&src_path)
                        .move_(
                            &gio::File::for_path(&dest),
                            gio::FileCopyFlags::OVERWRITE | gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                            Some(&cancel),
                            None,
                        )
                        .map_err(|e| e.to_string())
                } else if is_dir {
                    crate::utils::helpers::copy_dir_recursive_progress(
                        &src_path,
                        &dest,
                        &cancel,
                        task_id,
                        s.input_sender(),
                    )
                    .map_err(|e| e.to_string())
                } else {
                    crate::utils::helpers::copy_file_progress(
                        &src_path,
                        &dest,
                        &cancel,
                        task_id,
                        s.input_sender(),
                    )
                    .map_err(|e| e.to_string())
                };

                if let Err(e) = result {
                    if !e.contains("cancelled") && !e.contains("Cancelled") {
                        let template = crate::i18n::tr("Copy failed: {}");
                        let msg = template.replace("{}", &e);
                        s.input(AppMsg::ShowToast(msg));
                    }
                }

                s.input(AppMsg::TaskCompleted(task_id));

                let count = completed_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if count == total_files {
                    s.input(AppMsg::Refresh);
                }
            });
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
    /// Executes a shell command synchronously, blocking until the child process exits.
    ///
    /// Identical to [`run_custom_command`] except it calls `.status()` instead of `.spawn()`,
    /// guaranteeing the process has fully completed before returning. Must only be called
    /// from within a `spawn_blocking` context. Used when subsequent work (e.g. a UI refresh)
    /// must be causally ordered after the command, for example, restoring a file from trash.
    ///
    /// # Arguments
    ///
    /// * `command_template` - A shell command string with optional `%p`, `%d`, `%f` placeholders.
    /// * `file_path` - The target file path used to expand the placeholders.
    pub fn run_custom_command_wait(command_template: &str, file_path: &Path) {
        let path_str = file_path.to_string_lossy();
        let parent = file_path.parent().unwrap_or(file_path).to_string_lossy();
        let filename = file_path.file_name().unwrap_or_default().to_string_lossy();

        // Escape variables to prevent shell injection
        let p_arg = format!("'{}'", path_str.replace("'", "'\\''"));
        let d_arg = format!("'{}'", parent.replace("'", "'\\''"));
        let f_arg = format!("'{}'", filename.replace("'", "'\\''"));

        let mut final_cmd = command_template
            .replace("%p", &p_arg)
            .replace("%d", &d_arg)
            .replace("%f", &f_arg);

        // MANUALLY EXPAND ~ and $HOME:
        // This ensures that even if the Desktop environment has a limited PATH,
        // we resolve the user's local bin folder correctly.
        //Resolve tildes in the command template ONLY,
        // or better yet, rely on the shell to handle standard shortcuts.
        if final_cmd.starts_with("~/") {
            if let Ok(home) = std::env::var("HOME") {
                final_cmd = final_cmd.replacen("~", &home, 1);
            }
        }

        let _ = Command::new("sh").arg("-c").arg(final_cmd).status();
    }
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

/// Synchronously walks a path and sums its total size in bytes.
///
/// Used to populate the dialog denominator before I/O begins. Non-fatal:
/// unreadable entries are silently skipped.
pub fn scan_total_bytes(path: &std::path::Path) -> u64 {
    if path.is_file() {
        return std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    if path.is_dir() {
        let mut total = 0u64;
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                total += scan_total_bytes(&entry.path());
            }
        }
        return total;
    }
    0
}

/// Copies a single file via `gio::File::copy`, emitting `AppMsg::TaskProgress`
/// on each byte-level GIO callback.
///
/// The `cancellable` passed here is the same token registered in the task queue,
/// so cancelling via the transfer dialog actually aborts the I/O.
pub fn copy_file_progress(
    src: &std::path::Path,
    dest: &std::path::Path,
    cancellable: &gtk::gio::Cancellable,
    task_id: u64,
    sender: &relm4::Sender<crate::model::AppMsg>,
) -> Result<(), gtk::glib::Error> {
    use gtk::gio::prelude::*;
    let src_file = gtk::gio::File::for_path(src);
    let dst_file = gtk::gio::File::for_path(dest);
    let label = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let s = sender.clone();
    let lbl = label.clone();
    let c = cancellable.clone();

    src_file.copy(
        &dst_file,
        gtk::gio::FileCopyFlags::OVERWRITE | gtk::gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
        Some(cancellable),
        Some(&mut Box::new(move |current, total| {
            let _ = s.send(crate::model::AppMsg::TaskProgress {
                id: task_id,
                label: lbl.clone(),
                current: current as u64,
                total: total as u64,
                total_items: 1,
                cancellable: c.clone(),
            });
        })),
    )
}

/// Recursively copies a directory, threading the *same* cancellable through
/// every nested `gio::File::copy` call.
///
/// This ensures that cancelling via the transfer dialog actually aborts
/// in-progress directory copies (the old `copy_dir_recursive` used
/// `gio::Cancellable::NONE`, making cancellation a no-op).
pub fn copy_dir_recursive_progress(
    src: &std::path::Path,
    dest: &std::path::Path,
    cancellable: &gtk::gio::Cancellable,
    task_id: u64,
    sender: &relm4::Sender<crate::model::AppMsg>,
) -> std::io::Result<()> {
    use gtk::gio::prelude::*;

    if src == dest {
        return Ok(());
    }
    if !dest.exists() {
        std::fs::create_dir_all(dest)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;

        // Honour cancellation between files so we stop promptly.
        if cancellable.is_cancelled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "cancelled",
            ));
        }

        let child_src = entry.path();
        let child_dest = dest.join(entry.file_name());
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            copy_dir_recursive_progress(&child_src, &child_dest, cancellable, task_id, sender)?;
        } else {
            let label = child_src
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let s = sender.clone();
            let lbl = label.clone();
            let c = cancellable.clone();

            gtk::gio::File::for_path(&child_src)
                .copy(
                    &gtk::gio::File::for_path(&child_dest),
                    gtk::gio::FileCopyFlags::OVERWRITE | gtk::gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                    Some(cancellable),
                    Some(&mut Box::new(move |current, total| {
                        let _ = s.send(crate::model::AppMsg::TaskProgress {
                            id: task_id,
                            label: lbl.clone(),
                            current: current as u64,
                            total: total as u64,
                            total_items: 1,
                            cancellable: c.clone(),
                        });
                    })),
                )
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
    }

    Ok(())
}
