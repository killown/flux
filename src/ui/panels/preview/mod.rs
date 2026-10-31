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
//! The panel is split by responsibility. `content`, `state` and `handle` hold
//! what is rendered, the mutable view state and the public handle; `panel` and
//! `apply` build the widgets and fill them; and each viewer (`text`, `markdown`,
//! `table`, `pdf`, `audio`, `media`, `folder`, ...) owns its own format.
//! Everything outside this directory only sees the re-exports below.
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
