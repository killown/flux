use adw::prelude::*;
use relm4::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::typed_view::grid::TypedGridView;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use futures::StreamExt;

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
            add_controller = gtk::ShortcutController {
                // Shortcut: Ctrl + H -> Toggle Hidden Files
                add_shortcut = gtk::Shortcut {
                    set_trigger: Some(gtk::ShortcutTrigger::parse_string("<Control>h").unwrap()),
                    set_action: Some(gtk::CallbackAction::new(move |_, _| {
                        let _ = h_sender.input(AppMsg::ToggleHidden);
                        glib::Propagation::Stop
                    })),
                },
                // Shortcut: Ctrl + S -> Cycle Sort (Name -> Date -> Size)
                add_shortcut = gtk::Shortcut {
                    set_trigger: Some(gtk::ShortcutTrigger::parse_string("<Control>s").unwrap()),
                    set_action: Some(gtk::CallbackAction::new(move |_, _| {
                        let _ = s_sender.input(AppMsg::CycleSort);
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
                        set_title_widget = &gtk::Label {
                            add_css_class: "title-label",
                            #[watch] set_label: &model.current_path.display().to_string(),
                        },
                        // Sort Status Indicator
                        pack_end = &gtk::Label {
                            add_css_class: "sort-status-label",
                            #[watch] set_label: &format!("Sort: {:?}", model.sort_by),
                            set_margin_end: 12,
                            set_opacity: 0.7,
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
                                            if name.starts_with("/") {
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

        let config = utils::load_config();
        let (menu_model, menu_actions_map) = utils::load_menu_config(); 
        let context_menu_popover = gtk::PopoverMenu::from_model(Some(&menu_model));
        context_menu_popover.set_has_arrow(false);

        let action_group = gio::SimpleActionGroup::new();
        let app_sender = sender.clone();
        for (name, cmd) in menu_actions_map.iter() {
            let cmd_clone = cmd.clone();
            let sender_clone = app_sender.clone();
            let action = gio::SimpleAction::new(name, None);
            action.connect_activate(move |_, _| { 
                sender_clone.input(AppMsg::ExecuteCommand(cmd_clone.clone())); 
            });
            action_group.add_action(&action);
        }
        root.insert_action_group("win", Some(&action_group));

        let files = TypedGridView::<FileItem, gtk::SingleSelection>::new();
        let grid_view = &files.view;
        grid_view.set_max_columns(12);
        grid_view.set_min_columns(4);
        let sender_clone = sender.clone();
        grid_view.connect_activate(move |_, pos| sender_clone.input(AppMsg::Open(pos)));

        let listbox = gtk::ListBox::default();
        let sidebar = FactoryVecDeque::builder()
            .launch(listbox)
            .forward(sender.input_sender(), |path| AppMsg::Navigate(path));

        let volume_monitor = gio::VolumeMonitor::get();
        let s_added = sender.clone();
        volume_monitor.connect_mount_added(move |_, _| s_added.input(AppMsg::RefreshSidebar));
        let s_removed = sender.clone();
        volume_monitor.connect_mount_removed(move |_, _| s_removed.input(AppMsg::RefreshSidebar));
        let s_changed = sender.clone();
        volume_monitor.connect_drive_connected(move |_, _| s_changed.input(AppMsg::RefreshSidebar));

        let mut model = FluxApp {
            files,
            sidebar,
            current_path: start_path.clone(),
            history: Vec::new(),
            forward_stack: Vec::new(),
            load_id: Arc::new(AtomicU64::new(0)),
            current_icon_size: config.ui.default_icon_size,
            context_menu_popover,
            menu_actions: menu_actions_map,
            active_item_path: None,
            directory_monitor: None,
            action_group,
            sort_by: config.ui.default_sort.clone(),
            show_hidden: config.ui.show_hidden_by_default,
            config,
            _volume_monitor: volume_monitor,
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
            AppMsg::ToggleHidden => {
                self.show_hidden = !self.show_hidden;
                sender.input(AppMsg::Refresh);
            }
            AppMsg::CycleSort => {
                self.sort_by = match self.sort_by {
                    SortBy::Name => SortBy::Date,
                    SortBy::Date => SortBy::Size,
                    SortBy::Size => SortBy::Name,
                };
                sender.input(AppMsg::Refresh);
            }
            AppMsg::ShowContextMenu(x, y, path) => {
                self.active_item_path = path.clone();
                let is_bg = path.is_none();
                if is_bg { self.active_item_path = Some(self.current_path.clone()); }

                for (name, _) in &self.menu_actions {
                    let action_name: &str = name;
                    let should_enable = if is_bg {
                        action_name.contains("terminal") || action_name.contains("paste")
                    } else { true };

                    if let Some(action) = self.action_group.lookup_action(action_name) {
                        if let Some(simple_action) = action.downcast_ref::<gio::SimpleAction>() {
                            simple_action.set_enabled(should_enable);
                        }
                    }
                }
                self.context_menu_popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                self.context_menu_popover.popup();
            }
            AppMsg::ExecuteCommand(cmd_template) => {
                if let Some(path) = &self.active_item_path { utils::run_custom_command(&cmd_template, path); }
            }
            AppMsg::Zoom(delta) => {
                let change = if delta > 0.0 { -16 } else { 16 };
                let new_size = (self.current_icon_size + change).clamp(32, 256);
                if new_size != self.current_icon_size {
                    self.current_icon_size = new_size;
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
                if path.is_dir() {
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
                    let target = self.current_path.join(&item.name);
                    if target.is_dir() {
                        sender.input(AppMsg::Navigate(target));
                    } else {
                        utils::open_file(target);
                    }
                }
            }
        }
    }
}

impl FluxApp {
    pub fn refresh_sidebar(&mut self) {
        let mut guard = self.sidebar.guard();
        guard.clear();

        if let Some(p) = dirs::home_dir() { guard.push_back(SidebarPlace { name: "Home".to_string(), icon: "user-home-symbolic".to_string(), path: p }); }
        if let Some(p) = dirs::desktop_dir() { guard.push_back(SidebarPlace { name: "Desktop".to_string(), icon: "user-desktop-symbolic".to_string(), path: p }); }
        if let Some(p) = dirs::download_dir() { guard.push_back(SidebarPlace { name: "Downloads".to_string(), icon: "folder-download-symbolic".to_string(), path: p }); }
        if let Some(p) = dirs::document_dir() { guard.push_back(SidebarPlace { name: "Documents".to_string(), icon: "folder-documents-symbolic".to_string(), path: p }); }
        if let Some(p) = dirs::picture_dir() { guard.push_back(SidebarPlace { name: "Pictures".to_string(), icon: "folder-pictures-symbolic".to_string(), path: p }); }
        if let Some(p) = dirs::video_dir() { guard.push_back(SidebarPlace { name: "Videos".to_string(), icon: "folder-videos-symbolic".to_string(), path: p }); }

        if self.config.ui.show_xdg_dirs {
            guard.push_back(SidebarPlace { name: "Config".to_string(), icon: "emblem-system-symbolic".to_string(), path: utils::get_xdg_dir("XDG_CONFIG_HOME", "~/.config") });
            guard.push_back(SidebarPlace { name: "Local Data".to_string(), icon: "folder-remote-symbolic".to_string(), path: utils::get_xdg_dir("XDG_DATA_HOME", "~/.local/share") });
        }

        for custom in &self.config.sidebar {
            let path = if custom.path.starts_with('~') {
                dirs::home_dir().map(|h| PathBuf::from(custom.path.replace('~', &h.to_string_lossy()))).unwrap_or_else(|| PathBuf::from(&custom.path))
            } else {
                PathBuf::from(&custom.path)
            };
            guard.push_back(SidebarPlace { name: custom.name.clone(), icon: custom.icon.clone(), path });
        }

        for (name, path) in utils::get_system_mounts() {
            let icon = if name.to_lowercase().contains("drive") || name.to_lowercase().contains("cloud") || path.to_string_lossy().contains("Gdrive") {
                "folder-remote-symbolic".to_string()
            } else {
                "drive-harddisk-symbolic".to_string()
            };
            guard.push_back(SidebarPlace { name, icon, path });
        }
    }

    pub fn load_path(&mut self, path: PathBuf, sender: &ComponentSender<Self>) {
        self.directory_monitor = None;
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
                let metadata = entry.metadata().ok();
                items_metadata.push((name, target_path, metadata));
            }

            items_metadata.sort_by(|a, b| {
                let a_is_dir = a.2.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                let b_is_dir = b.2.as_ref().map(|m| m.is_dir()).unwrap_or(false);

                if a_is_dir != b_is_dir {
                    return b_is_dir.cmp(&a_is_dir);
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

            let mut media_tasks = Vec::new();
            for (name, target_path, metadata) in items_metadata {
                let is_dir = metadata.map(|m| m.is_dir()).unwrap_or(false);
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
