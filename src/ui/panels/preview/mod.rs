//! Peek preview side panel. Toggled by `Ctrl+Space` from the file grid,
//! follows selection while open.
//!
//! Every file gets a type-specific viewer: crisp PDF pages, full-resolution
//! images, syntect-highlighted text with a line-number gutter, Markdown,
//! CSV/TSV tables, hex dumps for binaries, folder / archive listings, or
//! media tags. Audio playback is delegated to `mpv` as a detached child
//! process. Video playback uses `gtk::Video` (GStreamer). The two paths are
//! deliberately kept separate because GStreamer's `decodebin3` aborts the
//! process on mislabeled containers, while mpv routes through FFmpeg.
//!
//! # Layout
//!
//! The panel is split by responsibility. Everything outside this directory only
//! sees the re-exports below.
//!
//! | Module        | Responsibility                                                     |
//! |---------------|--------------------------------------------------------------------|
//! | `content`     | `PreviewContent`, `FileInfo`, `FolderEntry`: what the panel renders |
//! | `state`       | `State` and `Mode`: mutable view state shared by handle clones      |
//! | `handle`      | `PreviewPanelHandle`, header-strip reflection, the Details tab      |
//! | `panel`       | `build_preview_panel`: widget tree and signal wiring                |
//! | `apply`       | `PreviewPanelHandle::apply`: content to widgets                     |
//! | `text`        | syntect / Markdown tag tables, byte to char offsets                 |
//! | `zoom`        | zoom handling and image layout                                      |
//! | `pdf`         | PDF paging and off-thread Poppler rasterisation                     |
//! | `audio`       | `mpv` child process and its IPC socket                              |
//! | `media`       | mute, stop and playlist mode for video and audio                    |
//! | `folder`      | folder icon grid, chunked fill and zoom                             |
//! | `widgets`     | small widget constructors and list helpers                          |
//! | `controllers` | event-controller helpers (menus, scroll, pan, double-click)         |
//! | `syntax`      | syntect highlighting and the dark / light tag palette               |
//! | `markdown`    | Markdown to styled ranges                                           |
//! | `table`       | CSV / TSV parsing and column / store construction                   |
//! | `gutter`      | line-number gutter `DrawingArea` for the text view                  |
//! | `binary`      | icon and metadata rows for PE / AppImage files                      |
//! | `office`      | office document to PDF conversion through `soffice`                 |
//! | `folder_thumbs` | lazy thumbnails for the folder grid                               |
//! | `registry`    | thread-local handle to the live panel                               |
//!
//! The format helpers are public (except `folder_thumbs` and `registry`,
//! which are crate-private) so the preview loader can call them as
//! `ui::panels::preview::{syntax, markdown, table, ...}`.

mod apply;
mod audio;
mod content;
mod controllers;
mod folder;
mod handle;
mod media;
mod panel;
mod pdf;
mod state;
mod text;
mod widgets;
mod zoom;

pub mod binary;
pub(crate) mod folder_thumbs;
pub mod gutter;
pub mod markdown;
pub mod office;
pub(crate) mod registry;
pub mod syntax;
pub mod table;

pub use content::{FileInfo, FolderEntry, PreviewContent};
pub use handle::PreviewPanelHandle;
pub use panel::build_preview_panel;
