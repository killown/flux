//! Mutable view state of the panel. One `State` is shared by every clone of the
//! panel handle through an `Rc`.

use super::content::FolderEntry;
use super::folder_thumbs::FolderThumbs;
use super::markdown::StyledRange;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    None,
    Pdf,
    Image,
    Text,
    Markdown,
    Table,
}

pub(crate) struct State {
    pub(super) mode: Cell<Mode>,
    /// Bumped on every content change, async results with an older token
    /// are discarded.
    pub(super) token: Cell<u64>,
    pub(super) path: RefCell<Option<PathBuf>>,
    pub(super) pages: Cell<usize>,
    pub(super) page: Cell<usize>,
    pub(super) zoom: Cell<f64>,
    pub(super) suppress: Cell<bool>,
    pub(super) rendered_w: Cell<i32>,
    pub(super) resize_gen: Cell<u64>,
    pub(super) tex_px: Cell<(i32, i32)>,
    pub(super) last_image_view: Cell<(i32, i32)>,
    pub(super) media: RefCell<Option<gtk::MediaFile>>,
    pub(super) pdf_path: RefCell<Option<PathBuf>>,
    /// gtk4-rs does not expose an iterator over `TextTagTable`, track the
    /// installed names ourselves so they can be removed on rebuild.
    pub(super) syntax_tags: RefCell<Vec<String>>,
    /// Cached rendered markdown so the source toggle can flip back without
    /// re-parsing.
    pub(super) md_cache: RefCell<Option<(String, Vec<StyledRange>)>>,
    /// Guards `mime_filter_btn` against re-entrant `connect_toggled` while
    /// its active state is being pushed from `FluxApp`.
    pub(super) mime_filter_suppress: Cell<bool>,
    /// Lazy image and video thumbnails of the folder grid.
    pub(super) folder_thumbs: FolderThumbs,
    /// Current icon size of the folder grid cells (Ctrl+scroll changes it).
    pub(super) folder_icon_size: Cell<i32>,
    /// Entries of the folder shown, kept so a zoom can refill the grid.
    pub(super) folder_entries: RefCell<Vec<FolderEntry>>,
    /// Bumped whenever the folder grid is refilled or released, a chunked fill
    /// still in progress stops when it no longer matches.
    pub(super) fill_gen: Cell<u64>,
    /// Playlist mode: when the playing video/audio ends, step to the next item.
    pub(super) playlist: Cell<bool>,
    /// Monotonic time (µs) until which non-media items are skipped after an auto-advance.
    pub(super) playlist_skip_until: Cell<i64>,
}
