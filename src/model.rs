use relm4::factory::FactoryVecDeque;
use relm4::typed_view::grid::TypedGridView;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::OnceLock;
use adw::gdk;
use gtk::gio;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ui_components::{FileItem, SidebarPlace};

pub static SENDER: OnceLock<relm4::Sender<AppMsg>> = OnceLock::new();

fn default_true() -> bool { true }

#[derive(Clone, Debug)]
pub struct CustomAction {
    pub label: String,
    pub action_name: String,
    pub command: String,
    pub mime_types: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub ui: UIConfig,
    pub sidebar: Vec<CustomPlace>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub enum SortBy {
    #[default]
    Name,
    Date,
    Size,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ContextAction {
    pub label: String,
    pub action_name: String,
    pub command: String,
    pub mime_types: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UIConfig {
    pub default_icon_size: i32,
    pub sidebar_width: i32,
    pub show_xdg_dirs: bool,
    #[serde(default)]
    pub default_sort: SortBy,
    #[serde(default = "default_true")]
    pub folders_first: bool,
    pub show_hidden_by_default: bool,
    pub show_xdg_dirs_by_default: bool,
    pub folder_sort: HashMap<String, SortBy>,
    pub folder_icon_size: HashMap<String, i32>,
    pub device_renames: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomPlace {
    pub name: String,
    pub icon: String,
    pub path: String,
}

#[derive(Debug)]
pub struct FluxApp {
    pub files: TypedGridView<FileItem, gtk::MultiSelection>,
    pub sidebar: FactoryVecDeque<SidebarPlace>,
    pub current_path: PathBuf,
    pub history: Vec<PathBuf>,
    pub forward_stack: Vec<PathBuf>,
    pub load_id: Arc<AtomicU64>,
    pub current_icon_size: i32,
    pub context_menu_popover: gtk::PopoverMenu,
    pub menu_actions: Vec<CustomAction>,
    pub active_item_path: Option<PathBuf>,
    pub directory_monitor: Option<gio::FileMonitor>,
    pub action_group: gio::SimpleActionGroup,
    pub sort_by: SortBy,
    pub show_hidden: bool,
    pub config: Config,
    pub _volume_monitor: gio::VolumeMonitor,
    pub filter: String,
    pub header_view: String,
}

#[derive(Debug)]
pub enum AppMsg {
    PrepareContextMenu(f64, f64, Option<PathBuf>),
    ShowContextMenu {
        x: f64,
        y: f64,
        path: Option<PathBuf>,
        mime: String,
    },
    #[allow(dead_code)]
    OpenFileProperties(PathBuf),
    Navigate(PathBuf),
    RefreshSidebar,
    ToggleHidden,
    CycleSort,
    CycleFolderPriority,
    UpdateFilter(String),
    ThumbnailReady {
        name: String,
        texture: gdk::Texture,
        load_id: u64,
    },
    SwitchHeader(String),
    ExecuteCommand(String),
    Zoom(f64),
    GoBack,
    GoForward,
    Refresh,
    Open(u32),
    EmptyTrash,
    #[allow(dead_code)]
    RestoreItem(PathBuf),
}
