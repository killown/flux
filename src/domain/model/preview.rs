//! Payloads the preview panel renders. They are built off the GTK thread by
//! `app::update::view::preview::compute_preview` and handed to
//! `PreviewPanelHandle::apply`.

use super::image::Image;
use std::ops::Range;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdStyle {
    Heading1,
    Heading2,
    Heading3,
    Heading4,
    Heading5,
    Heading6,
    Bold,
    Italic,
    Code,
    CodeBlock,
    Link,
    BlockQuote,
    ListBullet,
}

#[derive(Debug, Clone)]
pub struct StyledRange {
    pub range: Range<usize>,
    pub style: MdStyle,
}

/// Byte range inside the joined preview text that shares one visual style.
#[derive(Debug, Clone, Copy)]
pub struct HighlightedRange {
    pub start: usize,
    pub end: usize,
    pub fg: (u8, u8, u8),
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Debug, Clone)]
pub struct TableData {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
    pub delimiter: u8,
}

#[derive(Debug, Clone)]
pub enum PreviewContent {
    Loading,
    Image {
        image: Image,
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
        cover: Option<Image>,
        info: FileInfo,
    },
    Metadata {
        info: FileInfo,
    },
    Error(String),
}
