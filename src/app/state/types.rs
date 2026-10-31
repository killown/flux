//! Shared handles and caches of the running application.

use crate::domain::model::FileLoadContext;
use gtk::gdk;
use std::path::PathBuf;

/// Cached session data for a visited directory to avoid reloading items on revisit.
#[derive(Debug, Clone)]
pub struct CachedFolder {
    pub items: Vec<FileLoadContext>,
    pub media_tasks: Vec<(u32, PathBuf)>,
    pub thumbnails: std::collections::HashMap<PathBuf, gdk::Texture>,
    pub last_visited: std::time::Instant,
}
