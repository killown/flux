//! Clipboard-to-file paste operations for Flux.
//!
//! Extends the standard `AppMsg::Paste` handler to support saving non-file
//! clipboard content directly into the current directory:
//!
//! | Clipboard content              | Output file                              |
//! |--------------------------------|------------------------------------------|
//! | PNG / JPEG image (no URI list) | `clipboard_<timestamp>.png`              |
//! | Plain text (no URI list)       | `clipboard_<timestamp>.txt`              |
//! | `text/html` rich text          | `clipboard_<timestamp>.html`             |
//!
//! The entry point is [`FluxApp::handle_paste_from_clipboard`], which
//! inspects clipboard formats before falling back to the URI-list path.

use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use gtk::gdk;
use gtk::gdk::prelude::{DisplayExt, TextureExt};
use gtk::gio;
use gtk::glib;
use relm4::prelude::*;
use std::path::{Path, PathBuf};

impl FluxApp {
    /// Inspects the GDK clipboard and dispatches the appropriate paste variant.
    ///
    /// Priority order:
    /// 1. `text/uri-list` - standard file-manager copy/cut path.
    /// 2. `image/png` or `image/jpeg` - save image buffer to disk.
    /// 3. `text/html` - save as `.html`.
    /// 4. `text/plain` - text-to-file (`.txt`).
    pub fn handle_paste_from_clipboard(&self, sender: &AsyncComponentSender<Self>) {
        let Some(display) = gdk::Display::default() else {
            sender.input(AppMsg::ShowToast(crate::i18n::tr(
                "No display available for clipboard operation",
            )));
            return;
        };

        let clipboard = display.clipboard();
        let formats = clipboard.formats();

        let has_uri_list = formats.contain_mime_type("text/uri-list");
        if has_uri_list {
            self.paste_uri_list(clipboard, sender);
            return;
        }

        let has_png = formats.contain_mime_type("image/png");
        let has_jpeg =
            formats.contain_mime_type("image/jpeg") || formats.contain_mime_type("image/jpg");

        if has_png || has_jpeg {
            self.paste_image(clipboard, sender);
            return;
        }

        if formats.contain_mime_type("text/html") {
            self.paste_html(clipboard, sender);
            return;
        }

        if formats.contain_mime_type("text/plain")
            || formats.contain_mime_type("text/plain;charset=utf-8")
        {
            self.paste_text(clipboard, sender);
            return;
        }

        sender.input(AppMsg::ShowToast(crate::i18n::tr(
            "Nothing pasteable in clipboard",
        )));
    }

    /// Handles standard URI-list paste operations.
    fn paste_uri_list(&self, clipboard: gdk::Clipboard, sender: &AsyncComponentSender<Self>) {
        let s = sender.clone();
        let paste_toast = self
            .menu_actions
            .iter()
            .find(|a| a.command == "builtin::paste")
            .and_then(|a| a.toast.clone());

        clipboard.read_text_async(None::<&gio::Cancellable>, move |res| {
            if let Ok(Some(text)) = res {
                let mut lines = text.lines();
                let first_line = lines.next().unwrap_or("");
                let is_cut = first_line == "cut";

                let files: Vec<gio::File> = lines
                    .filter(|uri| !uri.is_empty())
                    .map(|uri| gio::File::for_uri(uri.trim_end_matches('\r')))
                    .collect();

                if !files.is_empty() {
                    s.input(AppMsg::PerformPaste { files, is_cut });
                    if let Some(toast) = paste_toast {
                        s.input(AppMsg::ShowToast(toast));
                    }
                }
            }
        });
    }

    /// Reads an image buffer from the clipboard and writes it to the active directory.
    fn paste_image(&self, clipboard: gdk::Clipboard, sender: &AsyncComponentSender<Self>) {
        let dest_dir = self.current_path.clone();
        let s = sender.clone();

        clipboard.read_texture_async(None::<&gio::Cancellable>, move |res| match res {
            Ok(Some(texture)) => {
                let file_name = timestamped_name("clipboard", "png");
                let dest = unique_path(&dest_dir, &file_name);

                match texture.save_to_png(dest.to_string_lossy().as_ref()) {
                    Ok(()) => {
                        s.input(AppMsg::ShowToast(crate::i18n::tr("Image pasted as file")));
                        s.input(AppMsg::Refresh);
                    }
                    Err(e) => {
                        s.input(AppMsg::ShowToast(format!(
                            "{}: {}",
                            crate::i18n::tr("Failed to save image"),
                            e
                        )));
                    }
                }
            }
            Ok(None) => {
                s.input(AppMsg::ShowToast(crate::i18n::tr(
                    "Could not read image from clipboard",
                )));
            }
            Err(e) => {
                s.input(AppMsg::ShowToast(format!(
                    "{}: {}",
                    crate::i18n::tr("Clipboard read error"),
                    e
                )));
            }
        });
    }

    /// Reads HTML markup from the clipboard and writes it as a `.html` file.
    fn paste_html(&self, clipboard: gdk::Clipboard, sender: &AsyncComponentSender<Self>) {
        let dest_dir = self.current_path.clone();
        let s = sender.clone();

        clipboard.read_async(
            &["text/html"],
            glib::Priority::DEFAULT,
            None::<&gio::Cancellable>,
            move |res| match res {
                Ok((stream, _actual_mime)) => {
                    read_stream_async_into_bytes(stream, dest_dir, s);
                }
                Err(e) => {
                    s.input(AppMsg::ShowToast(format!(
                        "{}: {}",
                        crate::i18n::tr("Clipboard read error"),
                        e
                    )));
                }
            },
        );
    }

    /// Reads plain text from the clipboard and writes it to a `.txt` file.
    fn paste_text(&self, clipboard: gdk::Clipboard, sender: &AsyncComponentSender<Self>) {
        let dest_dir = self.current_path.clone();
        let s = sender.clone();

        clipboard.read_text_async(None::<&gio::Cancellable>, move |res| {
            if let Ok(Some(text)) = res {
                let text = text.trim().to_string();
                if text.is_empty() {
                    return;
                }

                let file_name = timestamped_name("clipboard", "txt");
                let dest = unique_path(&dest_dir, &file_name);

                relm4::spawn_blocking(move || match std::fs::write(&dest, text.as_bytes()) {
                    Ok(()) => {
                        s.input(AppMsg::ShowToast(crate::i18n::tr("Text pasted as file")));
                        s.input(AppMsg::Refresh);
                    }
                    Err(e) => {
                        s.input(AppMsg::ShowToast(format!(
                            "{}: {}",
                            crate::i18n::tr("Failed to save text"),
                            e
                        )));
                    }
                });
            }
        });
    }
}

/// Generates a timestamped filename: `<stem>_YYYYMMDD_HHMM.<ext>`.
fn timestamped_name(stem: &str, ext: &str) -> String {
    let now = chrono::Local::now();
    format!("{}_{}.{}", stem, now.format("%Y%m%d_%H%M%S"), ext)
}

/// Returns a path that does not exist yet by appending `_2`, `_3`, … before the extension.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }

    let stem = Path::new(name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ext = Path::new(name)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    for n in 2u32.. {
        let numbered = dir.join(format!("{}_{}{}", stem, n, ext));
        if !numbered.exists() {
            return numbered;
        }
    }
    candidate
}

/// Reads a `gio::InputStream` fully using async chunk reads.
fn read_stream_async_into_bytes(
    stream: gio::InputStream,
    dest_dir: std::path::PathBuf,
    sender: relm4::AsyncComponentSender<crate::model::FluxApp>,
) {
    read_stream_chunk(stream, Vec::new(), dest_dir, sender);
}

fn read_stream_chunk(
    stream: gio::InputStream,
    mut acc: Vec<u8>,
    dest_dir: std::path::PathBuf,
    sender: relm4::AsyncComponentSender<crate::model::FluxApp>,
) {
    let stream_for_closure = stream.clone();
    stream.read_bytes_async(
        8192,
        glib::Priority::DEFAULT,
        None::<&gio::Cancellable>,
        move |res| match res {
            Ok(bytes) if bytes.is_empty() => {
                if acc.is_empty() {
                    sender.input(AppMsg::ShowToast(crate::i18n::tr(
                        "Could not read HTML from clipboard",
                    )));
                    return;
                }
                let html = String::from_utf8_lossy(&acc).into_owned();
                let file_name = timestamped_name("clipboard", "html");
                let dest = unique_path(&dest_dir, &file_name);
                let s = sender.clone();
                relm4::spawn_blocking(move || match std::fs::write(&dest, html.as_bytes()) {
                    Ok(()) => {
                        s.input(AppMsg::ShowToast(crate::i18n::tr("HTML pasted as file")));
                        s.input(AppMsg::Refresh);
                    }
                    Err(e) => {
                        s.input(AppMsg::ShowToast(format!(
                            "{}: {}",
                            crate::i18n::tr("Failed to save HTML"),
                            e
                        )));
                    }
                });
            }
            Ok(bytes) => {
                acc.extend_from_slice(&bytes);
                read_stream_chunk(stream_for_closure, acc, dest_dir, sender);
            }
            Err(e) => {
                sender.input(AppMsg::ShowToast(format!(
                    "{}: {}",
                    crate::i18n::tr("Clipboard read error"),
                    e
                )));
            }
        },
    );
}
