use adw::prelude::*;
use relm4::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::typed_view::grid::TypedGridView;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use futures::StreamExt;
use crate::file_properties::FileProperties;

use crate::ui_components::{FileItem, SidebarPlace};
use crate::utils;
use crate::model::{FluxApp, AppMsg, SortBy};
use adw::gdk;
use gtk::glib;
use gtk::gio;

#[relm4::component(pub)]
impl SimpleComponent for FluxApp {
    type Init = PathBuf;
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::Window {
            set_default_size: (1100, 750),
            set_title: Some("flux"),
            add_controller = gtk::EventControllerKey {
                connect_key_pressed[sender, header_view = model.header_view.clone()] => move |_, keyval, _, state| {
                    if header_view != "path" {
                        return glib::Propagation::Proceed;
                    }
                    if state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK | gdk::ModifierType::META_MASK) {
                        return glib::Propagation::Proceed;
                    }
                    if let Some(ch) = keyval.to_unicode() {
                        if ch.is_alphabetic() && !ch.is_control() {
                            sender.input(AppMsg::UpdateFilter(ch.to_string()));
                            sender.input(AppMsg::SwitchHeader("search".to_string()));
                            return glib::Propagation::Stop;
                        }
                    }
                    glib::Propagation::Proceed
                }
            },
            add_controller = gtk::ShortcutController {
                add_shortcut = gtk::Shortcut {
                    set_trigger: Some(gtk::ShortcutTrigger::parse_string("<Control>h").unwrap()),
                    set_action: Some(gtk::CallbackAction::new(move |_, _| {
                        let _ = h_sender.input(AppMsg::ToggleHidden);
                        glib::Propagation::Stop
                    })),
                },
                add_shortcut = gtk::Shortcut {
                    set_trigger: Some(gtk::ShortcutTrigger::parse_string("<Control>s").unwrap()),
                    set_action: Some(gtk::CallbackAction::new(move |_, _| {
                        let _ = s_sender.input(AppMsg::CycleSort);
                        glib::Propagation::Stop
                    })),
                },
                add_shortcut = gtk::Shortcut {
                    set_trigger: Some(gtk::ShortcutTrigger::parse_string("<Control>f").unwrap()),
                    set_action: Some(gtk::CallbackAction::new(move |_, _| {
                        let _ = f_sender.input(AppMsg::SwitchHeader("search".to_string()));
                        glib::Propagation::Stop
                    })),
                },
                add_shortcut = gtk::Shortcut {
                    set_trigger: Some(gtk::ShortcutTrigger::parse_string("<Shift>s").unwrap()),
                    set_action: Some(gtk::CallbackAction::new(move |_, _| {
                        let _ = s_sender_prio.input(AppMsg::CycleFolderPriority);
                        glib::Propagation::Stop
                    })),
                },
            },
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                #[name = "sidebar_container"]
                gtk::ScrolledWindow {
                    set_width_request: model.config.ui.sidebar_width,
                    add_css_class: "sidebar",
                },
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_hexpand: true,
                    adw::HeaderBar {
                        set_show_start_title_buttons: false,
                        set_show_end_title_buttons: false,
                        pack_start = &gtk::Button {
                            set_icon_name: "go-previous-symbolic",
                            connect_clicked => AppMsg::GoBack,
                            #[watch] set_sensitive: !model.history.is_empty(),
                        },
                        pack_start = &gtk::Button {
                            set_icon_name: "go-next-symbolic",
                            connect_clicked => AppMsg::GoForward,
                            #[watch] set_sensitive: !model.forward_stack.is_empty(),
                        },
                        #[wrap(Some)]
                        set_title_widget = &gtk::Stack {
                            #[watch] set_visible_child_name: &model.header_view,
                            set_transition_type: gtk::StackTransitionType::Crossfade,
                            add_child = &gtk::Button {
                                add_css_class: "flat",
                                #[watch] set_label: &model.current_path.to_string_lossy(),
                                connect_clicked => AppMsg::SwitchHeader("entry".to_string()),
                            } -> { set_name: "path" },
                            #[name = "path_entry"]
                            add_child = &gtk::Entry {
                                set_hexpand: false,
                                set_halign: gtk::Align::Center,
                                set_width_request: 450,
                                #[watch] set_text: &model.current_path.to_string_lossy(),
                                add_controller = gtk::EventControllerKey {
                                    connect_key_pressed[sender] => move |_, keyval, _, _| {
                                        if keyval == gdk::Key::Escape {
                                            sender.input(AppMsg::SwitchHeader("path".to_string()));
                                            return glib::Propagation::Stop;
                                        }
                                        glib::Propagation::Proceed
                                    }
                                },
                                connect_activate[sender] => move |entry| {
                                    let path_str = entry.text().to_string();
                                    if !path_str.is_empty() {
                                        sender.input(AppMsg::Navigate(PathBuf::from(path_str)));
                                    }
                                    sender.input(AppMsg::SwitchHeader("path".to_string()));
                                },
                                connect_show => |e| {
                                    e.grab_focus();
                                    e.set_position(-1);
                                }
                            } -> { set_name: "entry" },
                            add_child = &gtk::SearchEntry {
                                set_hexpand: false,
                                set_halign: gtk::Align::Center,
                                set_width_request: 450,
                                #[track(model.filter.is_empty())] set_text: &model.filter,
                                add_controller = gtk::EventControllerKey {
                                    connect_key_pressed[sender] => move |_, keyval, _, _| {
                                        if keyval == gdk::Key::Escape {
                                            sender.input(AppMsg::SwitchHeader("path".to_string()));
                                            return glib::Propagation::Stop;
                                        }
                                        glib::Propagation::Proceed
                                    }
                                },
                                connect_search_changed[sender] => move |entry| {
                                    sender.input(AppMsg::UpdateFilter(entry.text().to_string()));
                                },
                                connect_stop_search => AppMsg::SwitchHeader("path".to_string()),
                                add_controller = gtk::GestureClick {
                                    connect_pressed[sender] => move |_, _, _, _| {
                                        sender.input(AppMsg::SwitchHeader("entry".to_string()));
                                    }
                                },
                                connect_show => |e| {
                                    e.grab_focus();
                                    e.set_position(-1);
                                }
                            } -> { set_name: "search" },
                        },
                        pack_end = &gtk::Button {
                            #[watch]
                            set_visible: model.current_path.to_string_lossy() == "trash:///",
                            connect_clicked => AppMsg::EmptyTrash,
                            set_tooltip_text: Some("Empty Trash"),
                            add_css_class: "destructive-action",

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,

                                gtk::Image {
                                    set_icon_name: Some("user-trash-full-symbolic"),
                                },

                                gtk::Label {
                                    set_label: "Empty Trash",
                                }
                            }
                        },
                        pack_end = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,
                            set_margin_end: 12,
                            set_valign: gtk::Align::Center,
                            add_css_class: "sort-container",

                            gtk::Image {
                                set_icon_name: Some("view-sort-ascending-symbolic"),
                                set_pixel_size: 16,
                                set_opacity: 0.6,
                            },

                            gtk::Label {
                                add_css_class: "sort-status-label",
                                set_opacity: 0.8,
                                #[watch]
                                set_label: model.sort_status(),
                            }
                        }
                    },
                    #[name = "grid_scroller"]
                    gtk::ScrolledWindow { 
                        set_vexpand: true,
                        add_controller = gtk::EventControllerScroll {
                            set_flags: gtk::EventControllerScrollFlags::VERTICAL,
                            connect_scroll[sender] => move |ctrl, _, dy| {
                                let modifiers = ctrl.current_event_state();
                                if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                                    sender.input(AppMsg::Zoom(dy));
                                    return glib::Propagation::Stop;
                                }
                                glib::Propagation::Proceed
                            }
                        },
                        add_controller = gtk::GestureClick {
                            set_button: 3, 
                            connect_pressed[sender] => move |gesture, _, x, y| {
                                if let Some(widget) = gesture.widget() {
                                    let mut picked_path = None;
                                    if let Some(picked) = widget.pick(x, y, gtk::PickFlags::DEFAULT) {
                                        let mut current: Option<gtk::Widget> = Some(picked);
                                        while let Some(w) = current {
                                            let name = w.widget_name().to_string();
                                            if name.starts_with("/") || name.starts_with("trash://") {
                                                picked_path = Some(PathBuf::from(name));
                                                break;
                                            }
                                            current = w.parent();
                                        }
                                    }
                                    sender.input(AppMsg::ShowContextMenu(x, y, picked_path));
                                }
                            }
                        },
                    }
                }
            }
        }
    }

    fn init(start_path: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        relm4::set_global_css(include_str!("style.css"));

        let h_sender = sender.clone();
        let s_sender = sender.clone();
        let s_sender_prio = sender.clone();
        let f_sender = sender.clone();

        let config = utils::load_config();
 
        let menu_actions_list = utils::load_menu_config();
 
        let context_menu_popover = gtk::PopoverMenu::builder()
            .has_arrow(false)
            .build();

        let action_group = gio::SimpleActionGroup::new();
        let app_sender = sender.clone();

        let prio_action = gio::SimpleAction::new("cycle-priority", None);
        let prio_sender = sender.clone();
        prio_action.connect_activate(move |_, _| {
            prio_sender.input(AppMsg::CycleFolderPriority);
        });
        action_group.add_action(&prio_action);

        for action_def in &menu_actions_list {
            let cmd_clone = action_def.command.clone();
            let sender_clone = app_sender.clone();
            let action = gio::SimpleAction::new(&action_def.action_name, None);
            action.connect_activate(move |_, _| { 
                sender_clone.input(AppMsg::ExecuteCommand(cmd_clone.clone())); 
            });
            action_group.add_action(&action);
        }
        root.insert_action_group("win", Some(&action_group));

        let files = TypedGridView::<FileItem, gtk::MultiSelection>::new();
        files.view.set_enable_rubberband(true);

        let grid_view = &files.view;
        let sender_clone = sender.clone();
        grid_view.connect_activate(move |_, pos| sender_clone.input(AppMsg::Open(pos)));

        let listbox = gtk::ListBox::default();
        let sidebar = FactoryVecDeque::builder()
            .launch(listbox)
            .forward(sender.input_sender(), |path| AppMsg::Navigate(path));

        let volume_monitor = gio::VolumeMonitor::get();
        let s_added = sender.clone();
        volume_monitor.connect_mount_added(move |_, _| s_added.input(AppMsg::RefreshSidebar));
        let s_added_bis = sender.clone();
        volume_monitor.connect_mount_added(move |_, _| s_added_bis.input(AppMsg::RefreshSidebar));

        let mut model = FluxApp {
            files,
            sidebar,
            current_path: start_path.clone(),
            history: Vec::new(),
            forward_stack: Vec::new(),
            load_id: Arc::new(AtomicU64::new(0)),
            current_icon_size: config.ui.default_icon_size,
            context_menu_popover,
            menu_actions: menu_actions_list,
            active_item_path: None,
            directory_monitor: None,
            action_group,
            sort_by: config.ui.default_sort.clone(),
            show_hidden: config.ui.show_hidden_by_default,
            config,
            _volume_monitor: volume_monitor,
            filter: String::new(),
            header_view: "path".to_string(),
        };

        model.refresh_sidebar();
        model.load_path(start_path, &sender);

        let widgets = view_output!();
        widgets.grid_scroller.set_child(Some(&model.files.view));
        widgets.sidebar_container.set_child(Some(model.sidebar.widget()));
        model.context_menu_popover.set_parent(&widgets.grid_scroller);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppMsg::RefreshSidebar => {
                self.refresh_sidebar();
            }
            AppMsg::OpenFileProperties(path) => {
               let properties_win = FileProperties::builder()
                    .launch(path)
                    .detach();
                properties_win.widget().present();
            }
            AppMsg::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                self.config.ui.show_hidden_by_default = self.show_hidden;
                utils::save_config(&self.config);
                sender.input(AppMsg::Refresh);
            }
            AppMsg::CycleSort => {
                 self.sort_by = match self.sort_by {
                     SortBy::Name => SortBy::Date,
                     SortBy::Date => SortBy::Size,
                     SortBy::Size => SortBy::Name,
                 };
 
                 let path_str = self.current_path.to_string_lossy().to_string();
                 self.config.ui.folder_sort.insert(path_str, self.sort_by.clone());
                 utils::save_config(&self.config);
                 sender.input(AppMsg::Refresh);
            }
            AppMsg::CycleFolderPriority => {
                self.config.ui.folders_first = !self.config.ui.folders_first;
                utils::save_config(&self.config);
                let path = self.current_path.clone();
                self.load_path(path, &sender);
            }
            AppMsg::UpdateFilter(query) => {
                self.filter = query;
                sender.input(AppMsg::Refresh);
            }
            AppMsg::SwitchHeader(view_name) => {
                self.header_view = view_name;
                if self.header_view == "path" {
                    self.filter = String::new();
                    sender.input(AppMsg::Refresh);
                }
            }

            AppMsg::ShowContextMenu(x, y, path) => {
                self.active_item_path = path.clone();
                let is_in_trash = self.current_path.to_string_lossy().starts_with("trash://");

                let target_mime = if let Some(ref p) = path {
                    utils::get_mime_type(p)
                } else {
                    "inode/directory".to_string()
                };

                let menu = gio::Menu::new();

                for action in &self.menu_actions {
                    let mut matches = false;

                    if is_in_trash {
                        // ONLY allow actions specifically marked for 'trash'
                        if action.mime_types.contains(&"trash".to_string()) {
                            matches = true;
                        }
                    } else {
                        // ONLY allow standard actions if NOT in trash
                        for allowed_mime in &action.mime_types {
                            // Skip the trash-specific actions when browsing normal folders
                            if allowed_mime == "trash" { continue; }

                            matches = match allowed_mime.as_str() {
                                "*" | "all" => true,
                                "image/all" | "image/*" => target_mime.starts_with("image/"),
                                "video/all" | "video/*" => target_mime.starts_with("video/"),
                                "application/all" | "application/*" => target_mime.starts_with("application/"),
                                "text/all" | "text/*" => {
                                    target_mime.starts_with("text/") || 
                                    gio::content_type_is_a(&target_mime, "text/plain") ||
                                    target_mime == "inode/x-empty"
                                },
                                "folder" | "directory" => target_mime == "inode/directory",
                                "file" => target_mime != "inode/directory",
                                t => t == target_mime,
                            };
                            if matches { break; }
                        }
                    }

                    if matches {
                        let full_action_name = format!("win.{}", action.action_name);
                        menu.append(Some(&action.label), Some(&full_action_name));
                        if let Some(g_action) = self.action_group.lookup_action(&action.action_name) {
                            if let Some(simple) = g_action.downcast_ref::<gio::SimpleAction>() {
                                simple.set_enabled(true);
                            }
                        }
                    }
                }

                self.context_menu_popover.set_menu_model(Some(&menu));
                self.context_menu_popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                self.context_menu_popover.popup();
            }
            AppMsg::ExecuteCommand(cmd_template) => {
                let mut targets = Vec::new();

                if let Some(model) = self.files.view.model().and_then(|m| m.downcast::<gtk::MultiSelection>().ok()) {
                    let bitset = model.selection();
                    let n = bitset.size();
                    for i in 0..n {
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

                if final_targets.len() == 1 {
                    utils::run_custom_command(&cmd_template, &final_targets[0]);
                } else if !final_targets.is_empty() {
                    let paths_arg = final_targets.iter()
                        .map(|p| format!("'{}'", p.to_string_lossy().replace("'", "'\\''")))
                        .collect::<Vec<_>>()
                        .join(" ");

                    let mut cmd = cmd_template.replace("%p", &paths_arg);
                    if cmd.contains("%d") {
                         cmd = cmd.replace("%d", &format!("'{}'", self.current_path.to_string_lossy().replace("'", "'\\''")));
                    }

                    let _ = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(cmd)
                        .spawn();
                }
            }
            AppMsg::Zoom(delta) => {
                let change = if delta > 0.0 { -16 } else { 16 };
                let new_size = (self.current_icon_size + change).clamp(32, 512);
                if new_size != self.current_icon_size {
                    self.current_icon_size = new_size;

                    // SAVE CONFIGURATION FOR THIS FOLDER
                    let path_str = self.current_path.to_string_lossy().to_string();
                    self.config.ui.folder_icon_size.insert(path_str, new_size);
                    utils::save_config(&self.config);

                    for i in 0..self.files.len() {
                        if let Some(item_wrapper) = self.files.get(i as u32) {
                            let mut item = item_wrapper.borrow().clone();
                            item.icon_size = new_size;
                            self.files.remove(i as u32);
                            self.files.insert(i as u32, item);
                        }
                    }
                }
            }
            AppMsg::Navigate(path) => {
                let path_str = path.to_string_lossy();
                if path.is_dir() || path_str.starts_with("trash://") {
                    self.history.push(self.current_path.clone());
                    self.forward_stack.clear();

                    self.load_path(path, &sender);
                }
            }
            AppMsg::ThumbnailReady { name, texture, load_id } => {
                if load_id == self.load_id.load(Ordering::SeqCst) {
                    let target_idx = (0..self.files.len()).find(|&i| {
                        self.files.get(i as u32).map_or(false, |r| r.borrow().name == name)
                    });
                    if let Some(idx) = target_idx {
                        if let Some(item_wrapper) = self.files.get(idx as u32) {
                            let mut item = item_wrapper.borrow().clone();
                            item.thumbnail = Some(texture);
                            self.files.remove(idx as u32);
                            self.files.insert(idx as u32, item);
                        }
                    }
                }
            }
            AppMsg::GoBack => {
                if let Some(prev) = self.history.pop() {
                    self.forward_stack.push(self.current_path.clone());
                    self.load_path(prev, &sender);
                }
            }
            AppMsg::GoForward => {
                if let Some(next) = self.forward_stack.pop() {
                    self.history.push(self.current_path.clone());
                    self.load_path(next, &sender);
                }
            }
            AppMsg::Refresh => {
                let p = self.current_path.clone();
                self.load_path(p, &sender);
            }
            AppMsg::Open(index) => {
                if let Some(item_wrapper) = self.files.get(index) {
                    let item = item_wrapper.borrow();
                    let target = if self.current_path.to_string_lossy().starts_with("trash://") {
                        item.path.clone()
                    } else {
                        self.current_path.join(&item.name)
                    };
                    if target.is_dir() {
                        sender.input(AppMsg::Navigate(target));
                    } else {
                        utils::open_file(target);
                    }
                }
            }
            AppMsg::EmptyTrash => {
                let root = gio::File::for_uri("trash:///");
                if let Ok(enumerator) = root.enumerate_children("standard::name", gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE) {
                    for info in enumerator.flatten() {
                        let _ = root.child(info.name()).delete(gio::Cancellable::NONE);
                    }
                }
                sender.input(AppMsg::Refresh);
            }
            AppMsg::RestoreItem(_) => {
                sender.input(AppMsg::Refresh);
            }
        }
    }
}

impl FluxApp {
    fn sort_status(&self) -> &str {
        match self.sort_by {
            SortBy::Name => "Name",
            SortBy::Date => "Date",
            SortBy::Size => "Size",
        }
    }

    pub fn refresh_sidebar(&mut self) {
        let mut guard = self.sidebar.guard();
        guard.clear();

        let get_xdg_name = |p: &std::path::PathBuf| {
            gio::File::for_path(p)
                .query_info(gio::FILE_ATTRIBUTE_STANDARD_DISPLAY_NAME, gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
                .map(|info| info.display_name().to_string())
                .unwrap_or_else(|_| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
        };

        if let Some(p) = dirs::home_dir() { guard.push_back(SidebarPlace { name: get_xdg_name(&p), icon: "user-home-symbolic".to_string(), path: p }); }
        if let Some(p) = dirs::desktop_dir() { guard.push_back(SidebarPlace { name: get_xdg_name(&p), icon: "user-desktop-symbolic".to_string(), path: p }); }
        if let Some(p) = dirs::download_dir() { guard.push_back(SidebarPlace { name: get_xdg_name(&p), icon: "folder-download-symbolic".to_string(), path: p }); }
        if let Some(p) = dirs::document_dir() { guard.push_back(SidebarPlace { name: get_xdg_name(&p), icon: "folder-documents-symbolic".to_string(), path: p }); }
        if let Some(p) = dirs::picture_dir() { guard.push_back(SidebarPlace { name: get_xdg_name(&p), icon: "folder-pictures-symbolic".to_string(), path: p }); }
        if let Some(p) = dirs::video_dir() { guard.push_back(SidebarPlace { name: get_xdg_name(&p), icon: "folder-videos-symbolic".to_string(), path: p }); }

        if self.config.ui.show_xdg_dirs {
            guard.push_back(SidebarPlace { name: "Config".to_string(), icon: "emblem-system-symbolic".to_string(), path: utils::get_xdg_dir("XDG_CONFIG_HOME", "~/.config") });
            guard.push_back(SidebarPlace { name: "Local Data".to_string(), icon: "folder-remote-symbolic".to_string(), path: utils::get_xdg_dir("XDG_DATA_HOME", "~/.local/share") });
        }

        guard.push_back(SidebarPlace { name: "Trash".to_string(), icon: "user-trash-symbolic".to_string(), path: PathBuf::from("trash:///") });

        for custom in &self.config.sidebar {
            let path = if custom.path.starts_with('~') {
                dirs::home_dir().map(|h| PathBuf::from(custom.path.replace('~', &h.to_string_lossy()))).unwrap_or_else(|| PathBuf::from(&custom.path))
            } else {
                PathBuf::from(&custom.path)
            };
            guard.push_back(SidebarPlace { name: custom.name.clone(), icon: custom.icon.clone(), path });
        }

        for (mut name, path) in utils::get_system_mounts() {
                if let Some(new_name) = self.config.ui.device_renames.get(&name) {
                    name = new_name.clone();
                }

                let icon = if name.to_lowercase().contains("drive") || 
                              name.to_lowercase().contains("cloud") || 
                              path.to_string_lossy().contains("Gdrive") {
                    "folder-remote-symbolic".to_string()
                } else {
                    "drive-harddisk-symbolic".to_string()
                };
                guard.push_back(SidebarPlace { name, icon, path });
        }
    }

    pub fn load_path(&mut self, path: PathBuf, sender: &ComponentSender<Self>) {
        self.directory_monitor = None;
        let path_str = path.to_string_lossy().to_string();

        // 1. Sort Order
        if let Some(specific_sort) = self.config.ui.folder_sort.get(&path_str) {
            self.sort_by = specific_sort.clone();
        } else {
            self.sort_by = self.config.ui.default_sort.clone();
        }

        // 2. Icon Size
        if let Some(&size) = self.config.ui.folder_icon_size.get(&path_str) {
            self.current_icon_size = size;
        } else {
            self.current_icon_size = self.config.ui.default_icon_size;
        }

        // ----------------------------------------

        if path_str.starts_with("trash://") {
            self.files.clear();
            let root = gio::File::for_uri(&path_str);

            if let Ok(monitor) = root.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE) {
                let sender_clone = sender.clone();
                monitor.connect_changed(move |_, _, _, _| { sender_clone.input(AppMsg::Refresh); });
                self.directory_monitor = Some(monitor);
            }

            if let Ok(enumerator) = root.enumerate_children("standard::*", gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE) {
                for info in enumerator.flatten() {
                    let name = info.display_name().to_string();
                    let is_dir = info.file_type() == gio::FileType::Directory;
                    let child = root.child(info.name());
                    let child_path = PathBuf::from(child.uri());
                    self.files.append(FileItem {
                        name,
                        icon: info.icon().unwrap_or_else(|| gio::Icon::for_string("file").unwrap()),
                        thumbnail: None,
                        is_dir,
                        path: child_path,
                        icon_size: self.current_icon_size,
                    });
                }
            }
            self.current_path = path;
            return;
        }

        let file_obj = gio::File::for_path(&path);
        if let Ok(monitor) = file_obj.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE) {
            let sender_clone = sender.clone();
            monitor.connect_changed(move |_, _, _, _| { sender_clone.input(AppMsg::Refresh); });
            self.directory_monitor = Some(monitor);
        }

        self.files.clear();
        let current_session = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
        let session_arc = self.load_id.clone();

        if let Ok(entries) = std::fs::read_dir(&path) {
            let mut items_metadata = Vec::new();

            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !self.show_hidden && name.starts_with('.') { continue; }

                let target_path = path.join(&name);
                let is_dir = target_path.is_dir(); 
                let metadata = entry.metadata().ok();
                items_metadata.push((name, target_path, metadata, is_dir));
            }

            if !self.filter.is_empty() {
                let query = self.filter.to_lowercase();
                let matches: Vec<_> = items_metadata.iter()
                    .filter(|(name, ..)| name.to_lowercase().contains(&query))
                    .cloned()
                    .collect();
                if !matches.is_empty() {
                    items_metadata = matches;
                }
            }

            let folders_first = self.config.ui.folders_first;

            items_metadata.sort_by(|a, b| {
                let a_is_dir = a.3;
                let b_is_dir = b.3;

                if a_is_dir != b_is_dir {
                    return if folders_first {
                        b_is_dir.cmp(&a_is_dir)
                    } else {
                        a_is_dir.cmp(&b_is_dir)
                    };
                }

                match self.sort_by {
                    SortBy::Name => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
                    SortBy::Size => {
                        let a_size = a.2.as_ref().map(|m| m.len()).unwrap_or(0);
                        let b_size = b.2.as_ref().map(|m| m.len()).unwrap_or(0);
                        b_size.cmp(&a_size)
                    }
                    SortBy::Date => {
                        let a_time = a.2.as_ref().and_then(|m| m.modified().ok());
                        let b_time = b.2.as_ref().and_then(|m| m.modified().ok());
                        b_time.cmp(&a_time)
                    }
                }
            });

            let mut media_tasks: Vec<(String, PathBuf)> = Vec::new();
            for (name, target_path, _metadata, is_dir) in items_metadata {
                let icon = utils::get_icon_for_path(&target_path, is_dir);

                self.files.append(FileItem { 
                    name: name.clone(), 
                    icon, 
                    thumbnail: None, 
                    is_dir,
                    path: target_path.clone(),
                    icon_size: self.current_icon_size,
                });

                if !is_dir {
                    let (is_img, is_vid) = utils::is_visual_media(&target_path);
                    if is_img || is_vid {
                        if let Ok(abs_path) = target_path.canonicalize() {
                            media_tasks.push((name, abs_path));
                        }
                    }
                }
            }

            self.current_path = path;
            let sender = sender.clone();
            relm4::spawn(async move {
                let mut stream = futures::stream::iter(media_tasks).map(|(name, media_path)| {
                    let inner_sender = sender.clone();
                    let inner_session = session_arc.clone();
                    async move {
                        if inner_session.load(Ordering::SeqCst) != current_session { return; }
                        let res = tokio::task::spawn_blocking(move || { utils::get_or_create_thumbnail(&media_path) }).await;
                        if let Ok(Some(texture)) = res {
                            if inner_session.load(Ordering::SeqCst) == current_session {
                                inner_sender.input(AppMsg::ThumbnailReady { name, texture, load_id: current_session });
                            }
                        }
                    }
                }).buffer_unordered(4);
                while let Some(_) = stream.next().await { 
                    if session_arc.load(Ordering::SeqCst) != current_session { break; } 
                }
            });
        }
    }
}
