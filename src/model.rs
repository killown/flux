use relm4::factory::FactoryVecDeque;
use relm4::typed_view::grid::TypedGridView;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use adw::gdk;
use gtk::gio;
use serde::Deserialize;

use crate::ui_components::{FileItem, SidebarPlace};

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub ui: UIConfig,
    pub sidebar: Vec<CustomPlace>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct UIConfig {
    pub default_icon_size: i32,
    pub sidebar_width: i32,
    pub show_xdg_dirs: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CustomPlace {
    pub name: String,
    pub icon: String,
    pub path: String,
}

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
    pub config: Config,
    pub _volume_monitor: gio::VolumeMonitor,
}

#[derive(Debug)]
pub enum AppMsg {
    Navigate(PathBuf),
    RefreshSidebar,
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
