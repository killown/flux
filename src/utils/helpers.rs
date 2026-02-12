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
        sender: &ComponentSender<Self>,
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
                if !c.is_control() {
                    sender.input(AppMsg::SwitchHeader("search".to_string()));
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

        // 1. Core XDG Directories
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

        // 2. Trash
        guard.push_back(SidebarPlace {
            name: "Trash".to_string(),
            icon: "user-trash-symbolic".to_string(),
            path: PathBuf::from("trash:///"),
        });

        // 3. Custom Sidebar logic
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

        // 4. Mounts
        for (mut name, path) in utils::get_system_mounts() {
            if let Some(new_name) = self.config.ui.device_renames.get(&name) {
                name = new_name.clone();
            }

            let icon = if name.to_lowercase().contains("drive")
                || name.to_lowercase().contains("cloud")
                || path.to_string_lossy().contains("Gdrive")
            {
                "folder-remote-symbolic".to_string()
            } else {
                "drive-harddisk-symbolic".to_string()
            };

            guard.push_back(SidebarPlace { name, icon, path });
        }

        // 5. Exclusive List
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
    pub fn setup_shortcuts(controller: &gtk::ShortcutController, sender: &ComponentSender<Self>) {
        let shortcuts = [
            ("<Control>h", AppMsg::ToggleHidden),
            ("F1", AppMsg::ShowHelp),
            ("<Control>s", AppMsg::CycleSort),
            ("<Shift>s", AppMsg::CycleFolderPriority),
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
    pub fn setup_actions(&self, sender: &ComponentSender<Self>) {
        let prio_action = gio::SimpleAction::new("cycle-priority", None);
        let prio_sender = sender.clone();
        prio_action.connect_activate(move |_, _| {
            prio_sender.input(AppMsg::CycleFolderPriority);
        });
        self.action_group.add_action(&prio_action);

        // Register the parameterized action for the "Open With..." submenu
        let launch_action =
            gio::SimpleAction::new("launch-with", Some(glib::VariantTy::new("s").unwrap()));
        let launch_sender = sender.clone();
        launch_action.connect_activate(move |_, parameter| {
            if let Some(app_id) = parameter.and_then(|p| p.get::<String>()) {
                launch_sender.input(AppMsg::LaunchWithApp(app_id));
            }
        });
        self.action_group.add_action(&launch_action);

        for action_def in &self.menu_actions {
            let cmd_clone = action_def.command.clone();
            let sender_clone = sender.clone();
            let action = gio::SimpleAction::new(&action_def.action_name, None);
            action.connect_activate(move |_, _| {
                sender_clone.input(AppMsg::ExecuteCommand(cmd_clone.clone()));
            });
            self.action_group.add_action(&action);
        }
    }
}
