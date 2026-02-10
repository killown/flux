use crate::model::{AppMsg, FluxApp, PathSegment, SortBy};
use crate::ui_components::SidebarPlace;
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

        if modifiers == gdk::ModifierType::CONTROL_MASK {
            if keyval == gdk::Key::Page_Up {
                sender.input(AppMsg::PrevExclusive);
                return glib::Propagation::Stop;
            }
            if keyval == gdk::Key::Page_Down {
                sender.input(AppMsg::NextExclusive);
                return glib::Propagation::Stop;
            }
            if keyval == gdk::Key::Delete {
                sender.input(AppMsg::ClearExclusive);
                return glib::Propagation::Stop;
            }
        }

        // Add to Exclusive List
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

        // 3. Global Capture
        if modifiers.is_empty() {
            if let Some(c) = keyval.to_unicode() {
                if !c.is_control() {
                    sender.input(AppMsg::SwitchHeader("search".to_string()));
                    sender.input(AppMsg::SearchInput(c));
                    return glib::Propagation::Stop;
                }
            }
        }

        // 4. Context Keys
        match keyval {
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

        let mut components = Vec::new();
        let mut current_p = self.current_path.clone();

        // Walk up parents
        loop {
            let name = current_p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "/".to_string());

            components.push(PathSegment {
                name,
                path: current_p.clone(),
            });

            if !current_p.pop() {
                break;
            }
        }

        for segment in components.into_iter().rev() {
            guard.push_back(segment);
        }
    }

    /// Returns the filesystem path of the first currently selected item in the file grid.
    pub(crate) fn get_selected_path(&self) -> Option<PathBuf> {
        self.files
            .view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
            .and_then(|selection_model| {
                let selection = selection_model.selection();
                if selection.is_empty() {
                    return None;
                }
                let first_index = selection.nth(0);
                self.files
                    .get(first_index)
                    .map(|wrapper| wrapper.borrow().path.clone())
            })
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
