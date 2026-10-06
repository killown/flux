//! Application state, messages and persistent configuration.

mod app;
mod config;
mod defaults;
mod file_load;
mod msg;
mod types;

pub use app::FluxApp;
#[allow(unused_imports)]
pub use config::ContextAction;
pub use config::{
    Config, CustomAction, CustomPlace, DeviceRename, MenuEntry, ShortcutsConfig, TerminalConfig,
    ThumbnailTypes, UIConfig,
};
pub use file_load::FileLoadContext;
pub use msg::AppMsg;
#[allow(unused_imports)]
pub use types::ConflictResolver;
pub use types::{
    AppInit, BackgroundSlot, CachedFolder, PathSegment, RightPanelType, SortBy, SENDER,
};
