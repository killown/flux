use relm4::factory::FactoryVecDeque;
use relm4::typed_view::grid::TypedGridView;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use adw::gdk;
use gtk::gio;
use serde::{Deserialize, Serialize};

use crate::ui_components::{FileItem, SidebarPlace};

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
    #[serde(default)]
    pub folder_sort: std::collections::HashMap<String, SortBy>,
    #[serde(default)]
    pub show_hidden_by_default: bool,
    #[serde(default)]
    pub show_xdg_dirs_by_default: bool,
    #[serde(default)]
    pub device_renames: std::collections::HashMap<String, String>,
    #[serde(default = "default_true")]
    pub folders_first: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomPlace {
    pub name: String,
    pub icon: String,
    pub path: String,
}

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
    Navigate(PathBuf),
    RefreshSidebar,
    ToggleHidden,
    CycleSort,
    CycleFolderPriority,
    UpdateFilter(String),
    SwitchHeader(String),
    ShowContextMenu(f64, f64, Option<PathBuf>),
    ExecuteCommand(String),
    Zoom(f64),
    Refresh,
    Open(u32),
    GoBack,
    GoForward,
    EmptyTrash,
    RestoreItem(PathBuf),
    ThumbnailReady { name: String, texture: gdk::Texture, load_id: u64 },
}
