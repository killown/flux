//! MIME matching shared by the right-click popover and the Contextual Actions panel.

use crate::model::CustomAction;
use crate::ui::constants;
use gtk::gio;

/// Whether `action` applies to `mime`.
///
/// Each entry of `action.mime_types` may be a `+`-joined list, and an action
/// matches as soon as any single requirement matches. Both surfaces call this so
/// they always offer the same actions for the same file.
pub fn action_matches_mime(action: &CustomAction, mime: &str) -> bool {
    crate::hit!("action_matches_mime");
    action.mime_types.iter().any(|allowed_mime| {
        allowed_mime.split('+').any(|req| match req.trim() {
            "all" | constants::FILTER_ALL => true,
            "image/all" | "image/*" => mime.starts_with("image/"),
            "video/all" | "video/*" => mime.starts_with("video/"),
            "audio/all" | "audio/*" => mime.starts_with("audio/"),
            "font/all" | "font/*" => mime.starts_with("font/"),
            "application/all" | "application/*" => mime.starts_with("application/"),
            "text/all" | "text/*" => {
                mime.starts_with("text/")
                    || gio::content_type_is_a(mime, constants::MIME_TEXT)
                    || mime == constants::MIME_EMPTY
            }
            constants::FILTER_FOLDER | "directory" => mime == constants::MIME_DIR,
            constants::FILTER_FILE => mime != constants::MIME_DIR,
            t if t.ends_with('/') => mime.starts_with(t),
            t => t == mime,
        })
    })
}
