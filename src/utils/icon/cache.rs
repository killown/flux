use adw::gio;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
