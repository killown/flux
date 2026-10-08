use std::path::{Path, PathBuf};

/// Returns the theme icon base name for a standard XDG folder, or `None` if
/// `path` is not one of them.
pub fn get_default_xdg_folder_icon(path: &Path) -> Option<&'static str> {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let home = dirs::home_dir();

    // Guard: never assign an XDG folder icon to $HOME itself if the XDG path
    // is disabled - `$HOME` should keep its own `user-home` icon.
    if let Some(ref h) = home {
        if path == h || resolved == *h {
            return Some("user-home");
        }
    }

    let is_same = |dir: Option<PathBuf>| -> bool {
        if let Some(d) = dir {
            if let Some(ref h) = home {
                if d == *h {
                    return false;
                }
            }
            if path == d {
                return true;
            }
            if let Ok(canon) = d.canonicalize() {
                return resolved == canon;
            }
        }
        false
    };

    if is_same(dirs::download_dir()) {
        return Some("folder-download");
    }
    if is_same(dirs::document_dir()) {
        return Some("folder-documents");
    }
    if is_same(dirs::picture_dir()) {
        return Some("folder-pictures");
    }
    if is_same(dirs::video_dir()) {
        return Some("folder-videos");
    }
    if is_same(dirs::audio_dir()) {
        return Some("folder-music");
    }
    if is_same(dirs::desktop_dir()) {
        return Some("user-desktop");
    }
    if is_same(dirs::public_dir()) {
        return Some("folder-publicshare");
    }
    if is_same(dirs::template_dir()) {
        return Some("folder-templates");
    }

    None
}
