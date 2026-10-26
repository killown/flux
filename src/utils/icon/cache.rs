use adw::gio;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

thread_local! {
    pub(super) static THEMED_ICON_CACHE: RefCell<HashMap<String, gio::Icon>> =
        RefCell::new(HashMap::new());

    /// Decoded custom file-icon textures, keyed on `(path, mtime_secs)`.
    /// `mtime` is part of the key so replacing the icon file invalidates the
    /// entry on the next bind.
    pub(super) static FILE_ICON_TEXTURE_CACHE:
        RefCell<HashMap<(PathBuf, i64), gtk::gdk::Texture>> = RefCell::new(HashMap::new());
}

/// Clears the cached GIO icons so they can be re-resolved under a new GTK theme.
pub fn invalidate_themed_icon_cache() {
    THEMED_ICON_CACHE.with(|cache| {
        cache.borrow_mut().clear();
    });
}

/// Returns a cached texture if present, without decoding.
pub fn peek_cached_file_icon_texture(path: &Path) -> Option<gtk::gdk::Texture> {
    crate::hit!("peek_cached_file_icon_texture");
    let mtime = std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    FILE_ICON_TEXTURE_CACHE.with(|c| c.borrow().get(&(path.to_path_buf(), mtime)).cloned())
}

fn pending_file_icons() -> &'static Mutex<HashSet<PathBuf>> {
    static PENDING: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    PENDING.get_or_init(Default::default)
}

/// Releases a pending custom-icon decode on drop, so a failed or panicked
/// worker never blocks later retries.
pub struct FileIconDecodeGuard(Option<PathBuf>);

impl FileIconDecodeGuard {
    pub fn disarm(mut self) {
        self.0 = None;
    }
}

impl Drop for FileIconDecodeGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            end_file_icon_decode(&path);
        }
    }
}

/// Claims the decode of `path`. Returns `None` when one is already in flight.
pub fn begin_file_icon_decode(path: &Path) -> Option<FileIconDecodeGuard> {
    crate::hit!("begin_file_icon_decode");
    let mut pending = pending_file_icons()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    pending
        .insert(path.to_path_buf())
        .then(|| FileIconDecodeGuard(Some(path.to_path_buf())))
}

pub fn end_file_icon_decode(path: &Path) {
    crate::hit!("end_file_icon_decode");
    pending_file_icons()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(path);
}

pub fn insert_cached_file_icon_texture(path: &Path, texture: gtk::gdk::Texture) {
    crate::hit!("insert_cached_file_icon_texture");
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    FILE_ICON_TEXTURE_CACHE.with(|c| {
        c.borrow_mut().insert((path.to_path_buf(), mtime), texture);
    });
}
