use adw::prelude::*;
use relm4::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::typed_view::grid::TypedGridView;
use std::fs;
use std::io::Write; 
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use futures::StreamExt;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::ui_components::{FileItem, SidebarPlace};
use adw::gdk;
use gtk::gdk_pixbuf;
use gtk::glib;
use gtk::gio;
use gtk::prelude::*;

pub struct FluxApp {
    pub files: TypedGridView<FileItem, gtk::SingleSelection>,
    pub sidebar: FactoryVecDeque<SidebarPlace>,
    pub current_path: PathBuf,
    pub history: Vec<PathBuf>,
    pub forward_stack: Vec<PathBuf>,
    pub load_id: Arc<AtomicU64>,
    pub current_icon_size: i32,
    pub context_menu_popover: gtk::PopoverMenu,
    pub menu_actions: Vec<(String, String)>,
    pub active_item_path: Option<PathBuf>,
    pub directory_monitor: Option<gio::FileMonitor>,
    pub action_group: gio::SimpleActionGroup,
}

#[derive(Debug)]
pub enum AppMsg {
    Navigate(PathBuf),
    GoBack,
    GoForward,
    Refresh,
    Open(u32),
    Zoom(f64),
    ShowContextMenu(f64, f64, Option<PathBuf>),
    ExecuteCommand(String),
    ThumbnailReady { 
        name: String, 
        texture: gdk::Texture, 
        load_id: u64 
    },
}

impl FluxApp {
    fn ensure_config_file() -> PathBuf {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("flux");
        if !config_dir.exists() { let _ = fs::create_dir_all(&config_dir); }
        let config_path = config_dir.join("menu.rs");
        if !config_path.exists() {
            let default_config = r#""Open Terminal" => "alacritty --working-directory=%d"
"Copy Path" => "echo -n %p | wl-copy"
"Move to Trash" => "gio trash %p"
"Open in Code" => "code %p""#;
            if let Ok(mut file) = fs::File::create(&config_path) { let _ = file.write_all(default_config.as_bytes()); }
        }
        config_path
    }

    fn load_menu_config() -> (gio::Menu, Vec<(String, String)>) {
        let path = Self::ensure_config_file();
        let menu = gio::Menu::new();
        let mut actions = Vec::new();
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("//") || line.is_empty() { continue; }
                if let Some((label_part, cmd_part)) = line.split_once("=>") {
                    let label = label_part.trim().trim_matches('"').trim();
                    let cmd = cmd_part.trim().trim_matches(',').trim().trim_matches('"');
                    if !label.is_empty() && !cmd.is_empty() {
                        let action_name = label.to_lowercase().replace(" ", "_");
                        let full_action_name = format!("win.{}", action_name);
                        menu.append(Some(label), Some(&full_action_name));
                        actions.push((action_name, cmd.to_string()));
                    }
                }
            }
        }
        (menu, actions)
    }

    fn get_icon_for_path(path: &Path, is_dir: bool) -> adw::gio::Icon {
        if is_dir { return adw::gio::ThemedIcon::new("folder").upcast(); }
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
        adw::gio::content_type_get_icon(&content_type)
    }

    fn is_visual_media(path: &Path) -> (bool, bool) {
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
        (content_type.starts_with("image/"), content_type.starts_with("video/"))
    }

    fn open_file(path: PathBuf) {
        let file = adw::gio::File::for_path(&path);
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
        if let Some(app_info) = adw::gio::AppInfo::default_for_type(&content_type, false) {
            let _ = app_info.launch(&[file], None::<&adw::gio::AppLaunchContext>);
        } else {
            let _ = Command::new("xdg-open").arg(path).spawn();
        }
    }

    fn get_or_create_thumbnail(path: &Path) -> Option<gdk::Texture> {
        let cache_dir = dirs::cache_dir()?.join("flux").join("thumbnails");
        if let Err(_) = fs::create_dir_all(&cache_dir) { return None; }
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        let hash = hasher.finish();
        let cache_path = cache_dir.join(format!("{}.png", hash));
        if cache_path.exists() {
             let file = adw::gio::File::for_path(&cache_path);
             return gdk::Texture::from_file(&file).ok();
        }
        let (is_img, is_vid) = Self::is_visual_media(path);
        if is_img {
            match gdk_pixbuf::Pixbuf::from_file_at_scale(path, 256, 256, true) {
                Ok(pixbuf) => {
                    if let Some(path_str) = cache_path.to_str() { let _ = pixbuf.savev(path_str, "png", &[]); }
                    return Some(gdk::Texture::for_pixbuf(&pixbuf));
                },
                Err(_) => return None
            }
        } else if is_vid {
            let status = Command::new("ffmpeg").arg("-y").arg("-loglevel").arg("panic").arg("-i").arg(path)
                .arg("-ss").arg("00:00:01.000").arg("-vframes").arg("1").arg("-vf").arg("scale=256:-1").arg(&cache_path).status();
            if let Ok(s) = status {
                if s.success() && cache_path.exists() {
                     let file = adw::gio::File::for_path(&cache_path);
                     return gdk::Texture::from_file(&file).ok();
                }
            }
        }
        None
    }

    fn run_custom_command(command_template: &str, file_path: &Path) {
        let path_str = file_path.to_string_lossy();
        let parent = file_path.parent().unwrap_or(file_path).to_string_lossy();
        let filename = file_path.file_name().unwrap_or_default().to_string_lossy();
        let final_cmd = command_template
            .replace("%p", &format!("\"{}\"", path_str))
            .replace("%d", &format!("\"{}\"", parent))
            .replace("%f", &format!("\"{}\"", filename));
        println!("Executing: {}", final_cmd);
        let _ = Command::new("sh").arg("-c").arg(final_cmd).spawn();
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
        let current_size = self.current_icon_size;

        if let Ok(entries) = fs::read_dir(&path) {
            let mut items = Vec::new();
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') { continue; }
                let target_path = path.join(&name);
                let is_dir = target_path.is_dir(); 
                let icon = Self::get_icon_for_path(&target_path, is_dir);

                self.files.append(FileItem {
                    name: name.clone(),
                    icon: icon.clone(),
                    thumbnail: None,
                    is_dir,
                    path: target_path.clone(),
                    icon_size: current_size,
                });
                items.push((name, target_path, is_dir));
            }

            let mut media_tasks = Vec::new();
            for (name, target_path, is_dir) in items {
                if !is_dir {
                    let (is_img, is_vid) = Self::is_visual_media(&target_path);
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
                        let res = tokio::task::spawn_blocking(move || { Self::get_or_create_thumbnail(&media_path) }).await;
                        if let Ok(Some(texture)) = res {
                            if inner_session.load(Ordering::SeqCst) == current_session {
                                inner_sender.input(AppMsg::ThumbnailReady { name, texture, load_id: current_session });
                            }
                        }
                    }
                }).buffer_unordered(4);
                while let Some(_) = stream.next().await { if session_arc.load(Ordering::SeqCst) != current_session { break; } }
            });
        }
    }

    pub fn add_sidebar_place(&mut self, name: &str, icon: &str, path: PathBuf) {
        self.sidebar.guard().push_back(SidebarPlace { name: name.to_string(), icon: icon.to_string(), path });
    }
}

#[relm4::component(pub)]
impl SimpleComponent for FluxApp {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::Window {
            set_default_size: (1100, 750),
            set_title: Some("flux"),
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                #[name = "sidebar_container"]
                gtk::ScrolledWindow {
                    set_width_request: 240,
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
                        pack_end = &gtk::Button {
                            set_icon_name: "view-refresh-symbolic",
                            connect_clicked => AppMsg::Refresh,
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

    fn init(_: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        relm4::set_global_css(include_str!("style.css"));
        let settings = adw::gio::Settings::new("org.gnome.desktop.interface");
        let theme_name: String = settings.string("gtk-theme").into();
        std::env::set_var("GTK_THEME", &theme_name);
        let style_manager = adw::StyleManager::default();
        if theme_name.to_lowercase().contains("dark") {
            style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
        } else {
            style_manager.set_color_scheme(adw::ColorScheme::Default);
        }

        let (menu_model, menu_actions_map) = Self::load_menu_config();
        let context_menu_popover = gtk::PopoverMenu::from_model(Some(&menu_model));
        context_menu_popover.set_has_arrow(false);

        let action_group = gio::SimpleActionGroup::new();
        let app_sender = sender.clone();
        for (name, cmd) in &menu_actions_map {
            let cmd_clone = cmd.clone();
            let sender_clone = app_sender.clone();
            let action = gio::SimpleAction::new(name, None);
            action.connect_activate(move |_, _| { sender_clone.input(AppMsg::ExecuteCommand(cmd_clone.clone())); });
            action_group.add_action(&action);
        }
        root.insert_action_group("win", Some(&action_group));

        let files = TypedGridView::<FileItem, gtk::SingleSelection>::new();
        let grid_view = &files.view;
        grid_view.set_max_columns(12);
        grid_view.set_min_columns(4);
        grid_view.set_enable_rubberband(false);
        let sender_clone = sender.clone();
        grid_view.connect_activate(move |_, pos| sender_clone.input(AppMsg::Open(pos)));

        let listbox = gtk::ListBox::default();
        let sidebar = FactoryVecDeque::builder().launch(listbox).forward(sender.input_sender(), |path| AppMsg::Navigate(path));

        let home_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut model = FluxApp {
            files,
            sidebar,
            current_path: home_path.clone(),
            history: Vec::new(),
            forward_stack: Vec::new(),
            load_id: Arc::new(AtomicU64::new(0)),
            current_icon_size: 128,
            context_menu_popover,
            menu_actions: menu_actions_map,
            active_item_path: None,
            directory_monitor: None,
            action_group,
        };

        if let Some(p) = dirs::home_dir() { model.add_sidebar_place("Home", "user-home-symbolic", p); }
        if let Some(p) = dirs::desktop_dir() { model.add_sidebar_place("Desktop", "user-desktop-symbolic", p); }
        if let Some(p) = dirs::download_dir() { model.add_sidebar_place("Downloads", "folder-download-symbolic", p); }
        if let Some(p) = dirs::document_dir() { model.add_sidebar_place("Documents", "folder-documents-symbolic", p); }
        if let Some(p) = dirs::picture_dir() { model.add_sidebar_place("Pictures", "folder-pictures-symbolic", p); }
        if let Some(p) = dirs::video_dir() { model.add_sidebar_place("Videos", "folder-videos-symbolic", p); }
        model.add_sidebar_place("Trash", "user-trash-symbolic", PathBuf::from("/tmp"));

        model.load_path(home_path, &sender);
        let widgets = view_output!();
        widgets.grid_scroller.set_child(Some(&model.files.view));
        widgets.sidebar_container.set_child(Some(model.sidebar.widget()));
        model.context_menu_popover.set_parent(&widgets.grid_scroller);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppMsg::ShowContextMenu(x, y, path) => {
                self.active_item_path = path.clone();
                let is_bg = path.is_none();
                if is_bg { self.active_item_path = Some(self.current_path.clone()); }

                for (name, _) in &self.menu_actions {
                    let action_name = name;
                    let should_enable = if is_bg {
                        action_name.contains("terminal") || action_name.contains("paste") || action_name.contains("new")
                    } else { true };

                    if let Some(action) = self.action_group.lookup_action(action_name) {
                        if let Ok(simple_action) = action.downcast::<gio::SimpleAction>() {
                            simple_action.set_enabled(should_enable);
                        }
                    }
                }
                self.context_menu_popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                self.context_menu_popover.popup();
            }
            AppMsg::ExecuteCommand(cmd_template) => {
                if let Some(path) = &self.active_item_path { Self::run_custom_command(&cmd_template, path); }
            }
            AppMsg::Zoom(delta) => {
                let change = if delta > 0.0 { -16 } else { 16 };
                let new_size = (self.current_icon_size + change).clamp(32, 256);
                if new_size != self.current_icon_size {
                    self.current_icon_size = new_size;
                    for i in 0..self.files.len() {
                        if let Some(mut item) = self.files.get(i as u32).map(|r| r.borrow().clone()) {
                            item.icon_size = new_size;
                            self.files.remove(i as u32);
                            self.files.insert(i as u32, item);
                        }
                    }
                    let min_cols = (1000 / (new_size + 40)).max(2) as u32;
                    self.files.view.set_min_columns(min_cols);
                }
            }
            AppMsg::Refresh => { let p = self.current_path.clone(); self.load_path(p, &sender); }
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
                        if let Some(item_ref) = self.files.get(i as u32) { item_ref.borrow().name == name } else { false }
                    });
                    if let Some(idx) = target_idx {
                        if let Some(mut item) = self.files.get(idx as u32).map(|r| r.borrow().clone()) {
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
            AppMsg::Open(index) => {
                if let Some(item) = self.files.get(index) {
                    let target = self.current_path.join(&item.borrow().name);
                    if target.is_dir() { sender.input(AppMsg::Navigate(target)); } else { Self::open_file(target); }
                }
            }
        }
    }
}
