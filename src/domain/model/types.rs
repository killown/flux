use crate::domain::model::ConflictChoice;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// Type alias for the conflict resolution channel used in file copy/move operations.
pub type ConflictResolver = Arc<Mutex<Option<oneshot::Sender<(ConflictChoice, bool)>>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPanelType {
    Tag,
    Search,
    Diff,
    Location,
    Connect,
    Preview,
    Actions,
    Terminal,
    QuickList,
    /// File metadata panel (opened from the context menu's Properties entry).
    Properties,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundSlot {
    Window,
    SidebarLeft,
    SidebarRight,
}

/// Per-session window switches parsed from the command line.
///
/// Lives here rather than in `args.rs` because `AppInit` is compiled into both the
/// `flux` library and the `flux-fm` binary, and only `model` exists in both.
/// `flux::args` re-exports it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WindowOptions {
    /// Temporary CLI override to hide the sidebar without persisting to config.
    pub no_sidebar: bool,
    /// Temporary CLI override to hide the top header bar without persisting to config.
    pub no_header: bool,
    /// Temporary CLI override to hide the bottom status bar without persisting to config.
    pub no_statusbar: bool,
    /// Open the preview panel in wide mode once the first folder has loaded.
    pub preview: bool,
}

/// Startup parameters passed from the argument parser to the Relm4 component initializer.
pub struct AppInit {
    /// The dir the file manager should open on startup.
    pub start_path: PathBuf,
    /// If the user started flux with an archive file, open it right after init.
    pub open_archive: Option<PathBuf>,
    /// Pre-seeded list of directory paths to populate the quick-panel triage queue on startup.
    pub quick_list: Option<Vec<PathBuf>>,
    /// Optional tag search filter (e.g. `"#games"`) to seed into the search entry
    /// and activate on initial application startup.
    pub tag_search: Option<String>,
    /// Per-session window switches (`--no-sidebar`, `--preview`, ...).
    pub opts: WindowOptions,
}

/// Represents a single component of a filesystem path for breadcrumb navigation.
#[derive(Debug, Clone)]
pub struct PathSegment {
    /// The user-facing name of the specific directory in the path hierarchy.
    pub name: String,
    /// The full absolute path representing this specific segment's location.
    pub path: PathBuf,
    /// Characters shown in the breadcrumb label before it is ellipsized.
    pub max_chars: i32,
}

/// Available sorting criteria for the file view.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortBy {
    /// Sort items alphabetically by their display name.
    #[default]
    Name,
    /// Sort items by their last modified timestamp.
    Date,
    /// Sort items by their filesystem size in bytes.
    Size,
    /// Sort items by their file extension or MIME type.
    Type,
}

impl SortBy {
    /// Returns the key used as the state/target for the stateful `app.sort-field`
    /// radio action. The UI wraps it in a `GVariant` so GIO can render the matching
    /// menu item with a native radio checkmark.
    pub fn as_action_key(self) -> &'static str {
        match self {
            SortBy::Name => "name",
            SortBy::Date => "date",
            SortBy::Size => "size",
            SortBy::Type => "type",
        }
    }

    /// Reconstructs a [`SortBy`] from the variant key used by the `app.sort-field` action.
    pub fn from_action_key(key: &str) -> Self {
        match key {
            "date" => SortBy::Date,
            "size" => SortBy::Size,
            "type" => SortBy::Type,
            _ => SortBy::Name,
        }
    }
}
