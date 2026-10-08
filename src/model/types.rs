use crate::model::{AppMsg, FileLoadContext};
use crate::ui::conflict_policy::ConflictChoice;
use gtk::gdk;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// Global communication channel for sending messages to the main application loop from background threads.
pub static SENDER: OnceLock<relm4::Sender<AppMsg>> = OnceLock::new();

/// Type alias for the conflict resolution channel used in file copy/move operations.
pub type ConflictResolver = Arc<Mutex<Option<oneshot::Sender<(ConflictChoice, bool)>>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPanelType {
    Tag,
    Search,
    Diff,
    Location,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundSlot {
    Window,
    SidebarLeft,
    SidebarRight,
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
    /// Temporary CLI override to hide the sidebar without persisting to config.
    pub no_sidebar: bool,
    /// Temporary CLI override to hide the top header bar without persisting to config.
    pub no_header: bool,
    /// Temporary CLI override to hide the bottom status bar without persisting to config.
    pub no_statusbar: bool,
}

/// Represents a single component of a filesystem path for breadcrumb navigation.
#[derive(Debug, Clone)]
pub struct PathSegment {
    /// The user-facing name of the specific directory in the path hierarchy.
    pub name: String,
    /// The full absolute path representing this specific segment's location.
    pub path: PathBuf,
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
    /// Returns the `GVariant` string used as the state/target for the stateful
    /// `app.sort-field` radio action, enabling GIO to render the matching menu
    /// item with a native radio checkmark.
    pub fn as_action_state(self) -> gtk::glib::Variant {
        let key = match self {
            SortBy::Name => "name",
            SortBy::Date => "date",
            SortBy::Size => "size",
            SortBy::Type => "type",
        };
        gtk::glib::Variant::from(key)
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

/// Cached session data for a visited directory to avoid reloading items on revisit.
#[derive(Debug, Clone)]
pub struct CachedFolder {
    pub items: Vec<FileLoadContext>,
    pub media_tasks: Vec<(u32, PathBuf)>,
    pub thumbnails: std::collections::HashMap<PathBuf, gdk::Texture>,
    pub last_visited: std::time::Instant,
}
