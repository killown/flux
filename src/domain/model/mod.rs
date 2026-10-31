//! Application messages, persistent configuration and the pure data they carry.
//!
//! Nothing in `domain` may depend on `ui`, `infra`, `app` or a toolkit crate.
//! The `FluxApp` state that holds widgets and service handles lives in
//! `app::state`.

mod archive;
mod cancel;
mod config;
mod conflict;
mod defaults;
mod doc_config;
mod file_load;
mod git;
mod image;
mod modes;
mod msg;
mod network;
mod preview;
pub mod quick_groups;
pub mod quick_list;
mod search;
mod session;
mod types;
mod undo;

pub use archive::{ArchiveEntry, ArchiveError, PendingSaves};
pub use cancel::CancelHandle;
#[allow(unused_imports)]
pub use config::ContextAction;
pub use config::{
    Config, CustomAction, CustomPlace, DeviceRename, MenuEntry, ShortcutsConfig, TerminalConfig,
    TerminalPosition, ThumbnailTypes, UIConfig,
};
pub use conflict::{ConflictChoice, ConflictContext, ConflictPolicy};
pub use defaults::MAX_CONTENT_SEARCH_RESULTS;
#[allow(unused_imports)]
// used by `src/lib.rs` for `gen-docs`, not linked by the `flux-fm` binary alone
pub use doc_config::render_config_markdown;
pub use file_load::{set_now_unix, CellLabels, FileLoadContext};
pub use git::GitFileStatus;
pub use image::Image;
pub use modes::{AltCycle, FilterMode, NavigateMode};
pub use msg::{
    ActionsMsg, AppMsg, AppSidebarMsg, ArchiveMsg, CommandMsg, DeviceMsg, FileOpsMsg, GitMsg,
    IconsMsg, LoadingMsg, NavigationMsg, NetworkMsg, PreviewMsg, PropertiesMsg, QuickListMsg,
    SearchMsg, SettingsMsg, TabsMsg, TagsMsg, TasksMsg, TerminalMsg, ThumbnailsMsg, ToggleVerb,
    TransferMsg, UndoMsg, ViewMsg, WindowMsg, WindowVisibility,
};
pub use network::{NetworkAuthFlags, NetworkBookmark, NetworkCredentials};
pub use preview::{
    FileInfo, FolderEntry, HighlightedRange, MdStyle, PreviewContent, StyledRange, TableData,
};
pub use search::{AdvancedSearchParams, ExtensionMatch};
pub use session::{ArchiveState, Filters, GitState, QuickState};
#[allow(unused_imports)]
pub use types::ConflictResolver;
pub use types::{AppInit, BackgroundSlot, PathSegment, RightPanelType, SortBy, WindowOptions};
pub use undo::{FileOp, TrashedItem};
