//! Shared handles and caches of the running application.

use crate::domain::model::{AppMsg, FileLoadContext};
use gtk::gdk;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Global communication channel for sending messages to the main application loop from background threads.
pub static SENDER: OnceLock<relm4::Sender<AppMsg>> = OnceLock::new();

/// Cached session data for a visited directory to avoid reloading items on revisit.
#[derive(Debug, Clone)]
pub struct CachedFolder {
    pub items: Vec<FileLoadContext>,
    pub media_tasks: Vec<(u32, PathBuf)>,
    pub thumbnails: std::collections::HashMap<PathBuf, gdk::Texture>,
    pub last_visited: std::time::Instant,
}
