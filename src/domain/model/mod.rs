//! Application state, messages and persistent configuration.

mod app;
mod config;
mod defaults;
mod doc_config;
mod file_load;
mod msg;
pub mod quick_groups;
pub mod quick_list;
mod types;

pub use app::{AltCycle, FilterMode, FluxApp, NavigateMode};
#[allow(unused_imports)]
pub use config::ContextAction;
pub use config::{
    Config, CustomAction, CustomPlace, DeviceRename, MenuEntry, ShortcutsConfig, TerminalConfig,
    TerminalPosition, ThumbnailTypes, UIConfig,
};
#[allow(unused_imports)]
// used by `src/lib.rs` for `gen-docs`, not linked by the `flux-fm` binary alone
pub use doc_config::render_config_markdown;
pub use file_load::{set_now_unix, CellLabels, FileLoadContext};
pub use msg::{
    ActionsMsg, AppMsg, AppSidebarMsg, ArchiveMsg, CommandMsg, DeviceMsg, FileOpsMsg, GitMsg,
    IconsMsg, LoadingMsg, NavigationMsg, NetworkMsg, PreviewMsg, PropertiesMsg, QuickListMsg,
    SearchMsg, SettingsMsg, TabsMsg, TagsMsg, TasksMsg, TerminalMsg, ThumbnailsMsg, ToggleVerb,
    TransferMsg, UndoMsg, ViewMsg, WindowMsg, WindowVisibility,
};
#[allow(unused_imports)]
pub use types::ConflictResolver;
pub use types::{
    AppInit, BackgroundSlot, CachedFolder, PathSegment, RightPanelType, SortBy, WindowOptions,
    SENDER,
};
