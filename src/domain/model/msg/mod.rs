//! Application messages, one sub-enum per topic.

mod actions;
mod archive;
mod command;
mod device;
mod file_ops;
mod git;
mod icons;
mod loading;
mod navigation;
mod network;
mod preview;
mod properties;
mod quick_list;
mod search;
mod settings;
mod sidebar;
mod tabs;
mod tags;
mod tasks;
mod terminal;
mod thumbnails;
mod toggle;
mod transfer;
mod undo;
mod view;
mod window;

pub use actions::ActionsMsg;
pub use archive::ArchiveMsg;
pub use command::CommandMsg;
pub use device::DeviceMsg;
pub use file_ops::FileOpsMsg;
pub use git::GitMsg;
pub use icons::IconsMsg;
pub use loading::LoadingMsg;
pub use navigation::NavigationMsg;
pub use network::NetworkMsg;
pub use preview::PreviewMsg;
pub use properties::PropertiesMsg;
pub use quick_list::QuickListMsg;
pub use search::SearchMsg;
pub use settings::SettingsMsg;
pub use sidebar::AppSidebarMsg;
pub use tabs::TabsMsg;
pub use tags::TagsMsg;
pub use tasks::TasksMsg;
pub use terminal::TerminalMsg;
pub use thumbnails::ThumbnailsMsg;
pub use toggle::{ToggleVerb, WindowVisibility};
pub use transfer::TransferMsg;
pub use undo::UndoMsg;
pub use view::ViewMsg;
pub use window::WindowMsg;

/// Every event the application's update loop handles, grouped by topic.
#[derive(Debug, Clone)]
pub enum AppMsg {
    /// Actions panel, contextual actions, open-with and context menus.
    Actions(ActionsMsg),
    /// Archive browsing, extraction and passwords.
    Archive(ArchiveMsg),
    /// Command output dialog.
    Command(CommandMsg),
    /// Mounted devices and LUKS images.
    Device(DeviceMsg),
    /// Clipboard, create/rename/delete, drag and drop and file-change events.
    FileOps(FileOpsMsg),
    /// Git actions, status views and the diff panel.
    Git(GitMsg),
    /// Custom icons, MIME icon generation and icon settings.
    Icons(IconsMsg),
    /// Folder loading and cache.
    Loading(LoadingMsg),
    /// Navigation, history, recents and keyboard navigate mode.
    Navigation(NavigationMsg),
    /// Network locations and bookmarks.
    Network(NetworkMsg),
    /// Preview panel and pane focus.
    Preview(PreviewMsg),
    /// File properties and metadata.
    Properties(PropertiesMsg),
    /// Quick Lists and the exclusive navigation list.
    QuickList(QuickListMsg),
    /// Search, filters and the header search state.
    Search(SearchMsg),
    /// Persisted configuration setters.
    Settings(SettingsMsg),
    /// Sidebar sections, places and layout.
    Sidebar(AppSidebarMsg),
    /// Tab lifecycle.
    Tabs(TabsMsg),
    /// File tags and the tag panel.
    Tags(TagsMsg),
    /// Background task queue.
    Tasks(TasksMsg),
    /// Embedded terminal.
    Terminal(TerminalMsg),
    /// Thumbnail and video-preview generation.
    Thumbnails(ThumbnailsMsg),
    /// Transfer dialog and conflict resolution.
    Transfer(TransferMsg),
    /// Undo and redo.
    Undo(UndoMsg),
    /// Selection, sorting, zoom and view toggles.
    View(ViewMsg),
    /// Main window, toasts and help/about dialogs.
    Window(WindowMsg),
}

impl From<ActionsMsg> for AppMsg {
    fn from(m: ActionsMsg) -> Self {
        AppMsg::Actions(m)
    }
}

impl From<ArchiveMsg> for AppMsg {
    fn from(m: ArchiveMsg) -> Self {
        AppMsg::Archive(m)
    }
}

impl From<CommandMsg> for AppMsg {
    fn from(m: CommandMsg) -> Self {
        AppMsg::Command(m)
    }
}

impl From<DeviceMsg> for AppMsg {
    fn from(m: DeviceMsg) -> Self {
        AppMsg::Device(m)
    }
}

impl From<FileOpsMsg> for AppMsg {
    fn from(m: FileOpsMsg) -> Self {
        AppMsg::FileOps(m)
    }
}

impl From<GitMsg> for AppMsg {
    fn from(m: GitMsg) -> Self {
        AppMsg::Git(m)
    }
}

impl From<IconsMsg> for AppMsg {
    fn from(m: IconsMsg) -> Self {
        AppMsg::Icons(m)
    }
}

impl From<LoadingMsg> for AppMsg {
    fn from(m: LoadingMsg) -> Self {
        AppMsg::Loading(m)
    }
}

impl From<NavigationMsg> for AppMsg {
    fn from(m: NavigationMsg) -> Self {
        AppMsg::Navigation(m)
    }
}

impl From<NetworkMsg> for AppMsg {
    fn from(m: NetworkMsg) -> Self {
        AppMsg::Network(m)
    }
}

impl From<PreviewMsg> for AppMsg {
    fn from(m: PreviewMsg) -> Self {
        AppMsg::Preview(m)
    }
}

impl From<PropertiesMsg> for AppMsg {
    fn from(m: PropertiesMsg) -> Self {
        AppMsg::Properties(m)
    }
}

impl From<QuickListMsg> for AppMsg {
    fn from(m: QuickListMsg) -> Self {
        AppMsg::QuickList(m)
    }
}

impl From<SearchMsg> for AppMsg {
    fn from(m: SearchMsg) -> Self {
        AppMsg::Search(m)
    }
}

impl From<SettingsMsg> for AppMsg {
    fn from(m: SettingsMsg) -> Self {
        AppMsg::Settings(m)
    }
}

impl From<AppSidebarMsg> for AppMsg {
    fn from(m: AppSidebarMsg) -> Self {
        AppMsg::Sidebar(m)
    }
}

impl From<TabsMsg> for AppMsg {
    fn from(m: TabsMsg) -> Self {
        AppMsg::Tabs(m)
    }
}

impl From<TagsMsg> for AppMsg {
    fn from(m: TagsMsg) -> Self {
        AppMsg::Tags(m)
    }
}

impl From<TasksMsg> for AppMsg {
    fn from(m: TasksMsg) -> Self {
        AppMsg::Tasks(m)
    }
}

impl From<TerminalMsg> for AppMsg {
    fn from(m: TerminalMsg) -> Self {
        AppMsg::Terminal(m)
    }
}

impl From<ThumbnailsMsg> for AppMsg {
    fn from(m: ThumbnailsMsg) -> Self {
        AppMsg::Thumbnails(m)
    }
}

impl From<TransferMsg> for AppMsg {
    fn from(m: TransferMsg) -> Self {
        AppMsg::Transfer(m)
    }
}

impl From<UndoMsg> for AppMsg {
    fn from(m: UndoMsg) -> Self {
        AppMsg::Undo(m)
    }
}

impl From<ViewMsg> for AppMsg {
    fn from(m: ViewMsg) -> Self {
        AppMsg::View(m)
    }
}

impl From<WindowMsg> for AppMsg {
    fn from(m: WindowMsg) -> Self {
        AppMsg::Window(m)
    }
}
