use crate::model::{AppMsg, FluxApp};
use gtk::gio;
use gtk::gio::prelude::InputStreamExt;
use gtk::glib;
use relm4::prelude::*;

use super::util::{timestamped_name, unique_path};

/// Reads a `gio::InputStream` fully using async chunk reads and saves the
/// result to `<dest_dir>/clipboard_<timestamp>.html`.
pub(super) fn read_stream_async_into_bytes(
    stream: gio::InputStream,
    dest_dir: std::path::PathBuf,
    sender: relm4::AsyncComponentSender<FluxApp>,
) {
    read_stream_chunk(stream, Vec::new(), dest_dir, sender);
}

fn read_stream_chunk(
    stream: gio::InputStream,
    mut acc: Vec<u8>,
    dest_dir: std::path::PathBuf,
    sender: relm4::AsyncComponentSender<FluxApp>,
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
