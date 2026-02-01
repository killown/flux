use adw::prelude::*;
use relm4::prelude::*;
use relm4::factory::FactoryVecDeque;
use relm4::typed_view::grid::TypedGridView;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use futures::StreamExt;

use crate::ui_components::{FileItem, SidebarPlace};
use adw::gdk;

pub struct FluxApp {
    pub files: TypedGridView<FileItem, gtk::SingleSelection>,
    pub sidebar: FactoryVecDeque<SidebarPlace>,
    pub current_path: PathBuf,
    pub history: Vec<PathBuf>,
    pub forward_stack: Vec<PathBuf>,
    pub load_id: Arc<AtomicU64>,
}

#[derive(Debug)]
pub enum AppMsg {
    Navigate(PathBuf),
    GoBack,
    GoForward,
    Refresh,
    Open(u32),
    ThumbnailReady { 
        name: String, 
        texture: gdk::Texture, 
        load_id: u64 
    },
}

impl FluxApp {
    fn get_icon_for_path(path: &Path, is_dir: bool) -> adw::gio::Icon {
        if is_dir {
            return adw::gio::ThemedIcon::new("folder").upcast();
        }
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        let (content_type, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
        adw::gio::content_type_get_icon(&content_type)
    }

    fn is_image(path: &Path) -> bool {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        // Strict image extension list
        matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "gif" | "svg" | "bmp")
    }

    pub fn load_path(&mut self, path: PathBuf, sender: &ComponentSender<Self>) {
        self.files.clear();
        let current_session = self.load_id.fetch_add(1, Ordering::SeqCst) + 1;
        let session_arc = self.load_id.clone();

        if let Ok(entries) = fs::read_dir(&path) {
            let mut items = Vec::new();
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') { continue; }
                let target_path = path.join(&name);
                let is_dir = target_path.is_dir(); 
                let icon = Self::get_icon_for_path(&target_path, is_dir);

                items.push(FileItem { name, icon, thumbnail: None, is_dir });
            }

            // Sort: Folders first
            items.sort_by(|a, b| {
                if a.is_dir == b.is_dir {
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                } else {
                    b.is_dir.cmp(&a.is_dir)
                }
            });

            // Create tasks list AFTER sorting
            let mut image_tasks = Vec::new();
            for item in &items {
                let target_path = path.join(&item.name);
                if !item.is_dir && Self::is_image(&target_path) {
                    image_tasks.push((item.name.clone(), target_path));
                }
                self.files.append(FileItem { 
                    name: item.name.clone(), 
                    icon: item.icon.clone(), 
                    thumbnail: None, 
                    is_dir: item.is_dir 
                });
            }
            self.current_path = path;

            let sender = sender.clone();
            let session_ref = session_arc.clone();
            relm4::spawn(async move {
                let mut stream = futures::stream::iter(image_tasks)
                    .map(|(name, img_path)| {
                        let inner_sender = sender.clone();
                        let inner_session = session_ref.clone();
                        async move {
                            if inner_session.load(Ordering::SeqCst) != current_session {
                                return;
                            }

                            let res = tokio::task::spawn_blocking(move || {
                                let file = adw::gio::File::for_path(img_path);
                                gdk::Texture::from_file(&file).ok()
                            }).await;

                            if let Ok(Some(texture)) = res {
                                if inner_session.load(Ordering::SeqCst) == current_session {
                                    inner_sender.input(AppMsg::ThumbnailReady {
                                        name, // Send name back for lookup
                                        texture, 
                                        load_id: current_session
                                    });
                                }
                            }
                        }
                    })
                    .buffer_unordered(10);

                while let Some(_) = stream.next().await {
                    if session_ref.load(Ordering::SeqCst) != current_session {
                        break;
                    }
                }
            });
        }
    }

    pub fn add_sidebar_place(&mut self, name: &str, icon: &str, path: PathBuf) {
        self.sidebar.guard().push_back(SidebarPlace {
            name: name.to_string(),
            icon: icon.to_string(),
            path,
        });
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
                    gtk::ScrolledWindow { set_vexpand: true }
                }
            }
        }
    }

    fn init(_: (), _root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        relm4::set_global_css(include_str!("style.css"));

        let files = TypedGridView::<FileItem, gtk::SingleSelection>::new();
        let listbox = gtk::ListBox::default();
        listbox.set_activate_on_single_click(true);
        let sidebar = FactoryVecDeque::builder()
            .launch(listbox)
            .forward(sender.input_sender(), |path| AppMsg::Navigate(path));

        let home_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut model = FluxApp {
            files,
            sidebar,
            current_path: home_path.clone(),
            history: Vec::new(),
            forward_stack: Vec::new(),
            load_id: Arc::new(AtomicU64::new(0)),
        };

        if let Some(p) = dirs::home_dir() { model.add_sidebar_place("Home", "user-home-symbolic", p); }
        if let Some(p) = dirs::desktop_dir() { model.add_sidebar_place("Desktop", "user-desktop-symbolic", p); }
        if let Some(p) = dirs::download_dir() { model.add_sidebar_place("Downloads", "folder-download-symbolic", p); }
        if let Some(p) = dirs::document_dir() { model.add_sidebar_place("Documents", "folder-documents-symbolic", p); }
        if let Some(p) = dirs::picture_dir() { model.add_sidebar_place("Pictures", "folder-pictures-symbolic", p); }
        if let Some(p) = dirs::audio_dir() { model.add_sidebar_place("Music", "folder-music-symbolic", p); }
        if let Some(p) = dirs::video_dir() { model.add_sidebar_place("Videos", "folder-videos-symbolic", p); }
        model.add_sidebar_place("Trash", "user-trash-symbolic", PathBuf::from("/tmp"));

        model.load_path(home_path, &sender);
        let widgets = view_output!();
        let grid_view = &model.files.view;
        grid_view.set_max_columns(12);
        grid_view.set_min_columns(3);
        let sender_clone = sender.clone();
        grid_view.connect_activate(move |_, pos| sender_clone.input(AppMsg::Open(pos)));
        widgets.grid_scroller.set_child(Some(grid_view));
        widgets.sidebar_container.set_child(Some(model.sidebar.widget()));

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppMsg::Refresh => { 
                let p = self.current_path.clone(); 
                self.load_path(p, &sender); 
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
                    // SEARCH BY NAME INSTEAD OF INDEX
                    for i in 0..self.files.len() {
                        if let Some(item_handle) = self.files.get(i as u32) {
                            let mut item = item_handle.borrow_mut();
                            if item.name == name {
                                item.thumbnail = Some(texture);
                                break;
                            }
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
                    if target.is_dir() { 
                        sender.input(AppMsg::Navigate(target));
                    } else { 
                        let _ = Command::new("xdg-open").arg(target).spawn();
                    }
                }
            }
        }
    }
}
