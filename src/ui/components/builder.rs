use super::file_item::FileItem;
use crate::services::git::GitFileStatus;
use adw::gdk;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

impl FileItem {
    pub fn builder(name: String, path: PathBuf, icon: adw::gio::Icon) -> FileItemBuilder {
        FileItemBuilder {
            display_label: name.clone(),
            name,
            path,
            icon,
            is_dir: false,
            size: 0,
            mtime: 0,
            icon_size: 32,
            is_list_mode: false,
            max_width_chars: 20,
            grid_spacing: 10,
            show_symlink_emblem: true,
            grid_idx: 0,
            line_number: 0,
            is_cut: false,
            is_copy: false,
            is_empty: false,
            is_symlink: false,
            is_broken_symlink: false,
            expand_labels: false,
            is_foreign_owner: false,
            is_custom_icon: false,
            thumbnail: None,
            search_snippet: None,
            git_status: GitFileStatus::None,
            scale_font_with_icons: false,
            default_icon_size: 96,
            show_empty_dir_emblem: false,
            disable_drag_and_drop: false,
        }
    }
}

#[allow(dead_code)]
pub struct FileItemBuilder {
    name: String,
    path: PathBuf,
    icon: adw::gio::Icon,
    is_dir: bool,
    size: u64,
    mtime: i64,
    icon_size: i32,
    is_list_mode: bool,
    max_width_chars: i32,
    grid_spacing: i32,
    show_symlink_emblem: bool,
    grid_idx: u32,
    line_number: usize,
    is_cut: bool,
    is_copy: bool,
    is_empty: bool,
    is_symlink: bool,
    is_broken_symlink: bool,
    expand_labels: bool,
    is_foreign_owner: bool,
    is_custom_icon: bool,
    thumbnail: Option<gdk::Texture>,
    search_snippet: Option<String>,
    git_status: GitFileStatus,
    display_label: String,
    scale_font_with_icons: bool,
    default_icon_size: i32,
    show_empty_dir_emblem: bool,
    disable_drag_and_drop: bool,
}

#[allow(dead_code, clippy::wrong_self_convention)]
impl FileItemBuilder {
    pub fn is_dir(mut self, v: bool) -> Self {
        self.is_dir = v;
        self
    }

    pub fn size(mut self, v: u64) -> Self {
        self.size = v;
        self
    }

    pub fn mtime(mut self, v: i64) -> Self {
        self.mtime = v;
        self
    }

    pub fn icon_size(mut self, v: i32) -> Self {
        self.icon_size = v;
        self
    }

    pub fn is_list_mode(mut self, v: bool) -> Self {
        self.is_list_mode = v;
        self
    }

    pub fn max_width_chars(mut self, v: i32) -> Self {
        self.max_width_chars = v;
        self
    }

    pub fn grid_spacing(mut self, v: i32) -> Self {
        self.grid_spacing = v;
        self
    }

    pub fn show_symlink_emblem(mut self, v: bool) -> Self {
        self.show_symlink_emblem = v;
        self
    }

    pub fn grid_idx(mut self, v: u32) -> Self {
        self.grid_idx = v;
        self
    }

    pub fn line_number(mut self, v: usize) -> Self {
        self.line_number = v;
        self
    }

    pub fn is_cut(mut self, v: bool) -> Self {
        self.is_cut = v;
        self
    }

    pub fn is_copy(mut self, v: bool) -> Self {
        self.is_copy = v;
        self
    }

    pub fn is_empty(mut self, v: bool) -> Self {
        self.is_empty = v;
        self
    }

    pub fn is_symlink(mut self, v: bool) -> Self {
        self.is_symlink = v;
        self
    }

    pub fn is_broken_symlink(mut self, v: bool) -> Self {
        self.is_broken_symlink = v;
        self
    }

    pub fn expand_labels(mut self, v: bool) -> Self {
        self.expand_labels = v;
        self
    }

    pub fn is_foreign_owner(mut self, v: bool) -> Self {
        self.is_foreign_owner = v;
        self
    }

    pub fn is_custom_icon(mut self, v: bool) -> Self {
        self.is_custom_icon = v;
        self
    }

    pub fn thumbnail(mut self, v: Option<gdk::Texture>) -> Self {
        self.thumbnail = v;
        self
    }

    pub fn search_snippet(mut self, v: Option<String>) -> Self {
        self.search_snippet = v;
        self
    }

    pub fn git_status(mut self, v: GitFileStatus) -> Self {
        self.git_status = v;
        self
    }

    pub fn display_label(mut self, v: String) -> Self {
        self.display_label = v;
        self
    }

    pub fn scale_font_with_icons(mut self, v: bool) -> Self {
        self.scale_font_with_icons = v;
        self
    }

    pub fn default_icon_size(mut self, v: i32) -> Self {
        self.default_icon_size = v;
        self
    }

    pub fn show_empty_dir_emblem(mut self, v: bool) -> Self {
        self.show_empty_dir_emblem = v;
        self
    }

    pub fn disable_drag_and_drop(mut self, v: bool) -> Self {
        self.disable_drag_and_drop = v;
        self
    }

    pub fn build(self) -> FileItem {
        FileItem {
            name: self.name,
            icon: self.icon,
            size: self.size,
            thumbnail: self.thumbnail,
            is_dir: self.is_dir,
            path: self.path,
            icon_size: self.icon_size,
            is_editing: false,
            is_empty: self.is_empty,
            is_foreign_owner: self.is_foreign_owner,
            search_snippet: self.search_snippet,
            expand_labels: self.expand_labels,
            is_custom_icon: self.is_custom_icon,
            is_list_mode: self.is_list_mode,
            active_path: Rc::new(RefCell::new(None)),
            mtime: self.mtime,
            grid_idx: self.grid_idx,
            max_width_chars: self.max_width_chars,
            grid_spacing: self.grid_spacing,
            is_symlink: self.is_symlink,
            is_broken_symlink: self.is_broken_symlink,
            show_symlink_emblem: self.show_symlink_emblem,
            line_number: self.line_number,
            is_cut: self.is_cut,
            is_copy: self.is_copy,
            git_status: self.git_status,
            display_label: self.display_label,
            scale_font_with_icons: self.scale_font_with_icons,
            default_icon_size: self.default_icon_size,
            show_empty_dir_emblem: self.show_empty_dir_emblem,
            disable_drag_and_drop: self.disable_drag_and_drop,
        }
    }
}
