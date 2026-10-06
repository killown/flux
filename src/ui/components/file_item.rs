use crate::services::git::GitFileStatus;
use adw::gdk;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// Data model representing a file or directory entry in the application grid.
#[derive(Debug, Clone)]
pub struct FileItem {
    pub name: String,
    pub icon: adw::gio::Icon,
    pub size: u64,
    pub thumbnail: Option<gdk::Texture>,
    #[allow(dead_code)]
    pub is_dir: bool,
    pub path: PathBuf,
    pub icon_size: i32,
    pub is_editing: bool,
    pub is_empty: bool,
    pub is_foreign_owner: bool,
    pub search_snippet: Option<String>,
    /// Whether the label should wrap to multiple lines instead of ellipsizing.
    pub expand_labels: bool,
    /// Whether this icon was set by the user via F3 (custom folder icon).
    /// Used to determine if we should fall back to default when icon missing from theme.
    pub is_custom_icon: bool,
    /// When true the item renders as a compact horizontal row instead of the
    /// default vertical card (icon top, label bottom).
    pub is_list_mode: bool,
    /// Shared cell holding the current path for this item.
    /// Updated in `bind()` and read by the right‑click gesture.
    pub active_path: Rc<RefCell<Option<PathBuf>>>,
    // In the FileItem struct, add after `is_custom_icon`:
    /// Unix timestamp of the last modification time, `0` when unavailable.
    pub mtime: i64,
    /// Position of this item in the grid model.
    ///
    /// Set at construction time in `loader.rs` and read by `bind()` to dispatch
    /// lazy thumbnail requests without relying on widget-data that may not yet
    /// be populated.
    pub grid_idx: u32,
    /// Cached from config at construction time - avoids a config.toml read per bind() call.
    pub max_width_chars: i32,
    /// Cached from config at construction time - avoids a config.toml read per bind() call.
    pub grid_spacing: i32,
    /// Whether the target is a symbolic link.
    pub is_symlink: bool,
    /// Whether the symbolic link target does not exist.
    pub is_broken_symlink: bool,
    /// User setting controlling whether the visual symbolic link badge is shown.
    pub show_symlink_emblem: bool,
    /// Line number from a content-search hit. 0 for all other items.
    pub line_number: usize,
    /// Whether the item is currently cut (moved via clipboard).
    pub is_cut: bool,
    /// Indicates whether this item is currently marked in the clipboard for a copy operation to apply visual styling.
    pub is_copy: bool,
    /// Working tree status for git emblems and filters.
    pub git_status: GitFileStatus,
    pub display_label: String,
    pub scale_font_with_icons: bool,
    pub default_icon_size: i32,
    pub show_empty_dir_emblem: bool,
    pub disable_drag_and_drop: bool,
}
