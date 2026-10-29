//! Payloads the panel renders. They are built off the GTK thread by
//! `app::update::view::preview::compute_preview` and handed to
//! `PreviewPanelHandle::apply`.

use super::markdown::StyledRange;
use super::syntax::HighlightedRange;
use super::table::TableData;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct FileInfo {
    pub path: PathBuf,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub mtime: i64,
    pub extra: Vec<(String, String)>,
}

/// One entry of the folder grid shown for a directory.
#[derive(Debug, Clone)]
pub struct FolderEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Clone)]
pub enum PreviewContent {
    Loading,
    Image {
        tex: gtk::gdk::Texture,
        info: FileInfo,
    },
    Pdf {
        pages: usize,
        info: FileInfo,
        render_path: PathBuf,
    },
    Text {
        head: String,
        truncated: bool,
        hex: bool,
        info: FileInfo,
        highlighted: Vec<HighlightedRange>,
        language: Option<String>,
    },
    Markdown {
        rendered: String,
        styles: Vec<StyledRange>,
        source: String,
        info: FileInfo,
    },
    Table {
        data: TableData,
        info: FileInfo,
    },
    Folder {
        entries: Vec<FolderEntry>,
        more: usize,
        info: FileInfo,
    },
    Media {
        path: PathBuf,
        video: bool,
        cover: Option<gtk::gdk::Texture>,
        info: FileInfo,
    },
    Metadata {
        info: FileInfo,
    },
    Error(String),
}
