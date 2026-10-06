//! File actions: item activation, moves and clipboard export.

use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use relm4::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

impl FluxApp {
    /// Returns true if the current path is inside an archive that supports full extraction.
    pub fn can_extract_current_archive(&self) -> bool {
        let path_str = self.current_path.to_string_lossy();
        if !path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
            return false;
        }
        crate::services::archive::parse_archive_uri(&path_str)
            .map(|(archive_path, _)| {
                let name = archive_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();

                // Exclude ISO images and single-file stream formats (.gz, .xz, etc.)
                name.ends_with(".zip")
                    || name.ends_with(".7z")
                    || name.ends_with(".rar")
                    || name.ends_with(".tar")
                    || name.ends_with(".tar.gz")
                    || name.ends_with(".tgz")
                    || name.ends_with(".tar.bz2")
                    || name.ends_with(".tbz2")
                    || name.ends_with(".tar.xz")
                    || name.ends_with(".txz")
                    || name.ends_with(".tar.zst")
                    || name.ends_with(".tzst")
                    || name.ends_with(".tar.lz4")
                    || name.ends_with(".deb")
            })
            .unwrap_or(false)
    }

    /// Dispatches the correct action for each `(path, is_dir)` item pair.
    ///
    /// Centralises the routing logic shared by `AppMsg::Open` and `AppMsg::Activate`
    /// so that both code paths behave identically regardless of how the items were
    /// resolved (by grid position or by selection model).
    ///
    /// # Behaviour
    /// - `archive://` directory → `Navigate` into the virtual sub-directory.
    /// - `archive://` file → extract to a `NamedTempFile` and `xdg-open` it.
    /// - Real directory → `Navigate`.
    /// - Browsable archive on disk → `EnterArchive`.
    /// - Any other file → `open_file` (xdg-open).
    pub(crate) fn activate_items(
        &self,
        items: Vec<(PathBuf, bool)>,
        sender: &relm4::AsyncComponentSender<Self>,
    ) {
        #[allow(clippy::never_loop)]
        for (path, is_dir) in items {
            let path_str = path.to_string_lossy();

            // Check if this is an archive URI (/archive://... or archive://... or /archive:/...)
            if let Some((archive_path, inner)) =
                crate::services::archive::parse_archive_uri(&path_str)
            {
                if is_dir {
                    // Navigating deeper into a virtual archive folder
                    sender.input(AppMsg::Navigate(path));
                } else if crate::services::archive::is_browsable_archive(Path::new(&inner)) {
                    sender.input(AppMsg::EnterArchive(path));
                } else {
                    // Extracting and launching a file from inside the archive
                    let sender_clone = sender.clone();
                    let cached_pwd = self.cached_archive_password.clone();

                    let parent_prefix = Path::new(&inner)
                        .parent()
                        .and_then(|p| p.to_str())
                        .unwrap_or("")
                        .to_string();

                    relm4::spawn_blocking(move || {
                        match crate::services::archive::extract_entry_to_tempfile(
                            &archive_path,
                            &inner,
                            cached_pwd.as_deref(),
                        ) {
                            Ok(tmp) => {
                                if let Ok((_file, path_buf)) = tmp.keep() {
                                    Self::open_file(path_buf);
                                }
                            }
                            Err(crate::services::archive::ArchiveError::PasswordRequired) => {
                                sender_clone.input(AppMsg::PromptArchivePassword {
                                    archive_path,
                                    prefix: parent_prefix,
                                    wrong_password: false,
                                });
                            }
                            Err(crate::services::archive::ArchiveError::WrongPassword) => {
                                sender_clone.input(AppMsg::PromptArchivePassword {
                                    archive_path,
                                    prefix: parent_prefix,
                                    wrong_password: true,
                                });
                            }
                            Err(e) => {
                                sender_clone.input(AppMsg::ShowToast(e.to_string()));
                            }
                        }
                    });
                }
                break;
            }
            // Regular filesystem directory navigation
            else if is_dir {
                sender.input(AppMsg::Navigate(path));
                break;
            }
            // Opening a physical archive file from disk (.7z, .zip, etc.)
            else if crate::services::archive::is_browsable_archive(&path) {
                sender.input(AppMsg::EnterArchive(path));
                break;
            }
            // LUKS encrypted image file or standard file opening offloaded to background worker
            else {
                let sender_clone = sender.clone();
                let path_clone = path.clone();
                relm4::spawn_blocking(move || {
                    if crate::services::luks::is_luks_image(&path_clone) {
                        sender_clone.input(AppMsg::UnlockLuksImage { path: path_clone });
                    } else if path_clone.extension().and_then(|e| e.to_str()) == Some("desktop") {
                        if let Some(desktop_app) =
                            gio_unix::DesktopAppInfo::from_filename(&path_clone)
                        {
                            let app_name = desktop_app.display_name().to_string();
                            let context = gdk::Display::default().map(|d| d.app_launch_context());
                            match desktop_app.launch(&[], context.as_ref()) {
                                Ok(_) => {
                                    let msg = tr("Opening {}").replace("{}", &app_name);
                                    sender_clone.input(AppMsg::ShowToast(msg));
                                }
                                Err(e) => {
                                    eprintln!("[flux] Failed to launch desktop file: {e}");
                                    let msg = tr("Failed to open {}: {}")
                                        .replacen("{}", &app_name, 1)
                                        .replacen("{}", &e.to_string(), 1);
                                    sender_clone.input(AppMsg::ShowToast(msg));
                                }
                            }
                        } else {
                            Self::open_file(path_clone);
                        }
                    } else {
                        Self::open_file(path_clone);
                    }
                });
                break;
            }
        }
    }

    #[inline]
    pub fn is_dir_empty(path: &Path) -> bool {
        fs::read_dir(path)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
    }

    /// Moves a collection of source files/directories into a target destination directory.
    pub fn handle_move_files_to_target(
        &mut self,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let mut moved_any = false;
        for src in sources {
            if src == destination {
                continue;
            }
            if let Some(file_name) = src.file_name() {
                let dest_path = destination.join(file_name);
                if let Err(e) = std::fs::rename(&src, &dest_path) {
                    sender.input(AppMsg::ShowToast(format!("Failed to move file: {}", e)));
                } else {
                    moved_any = true;
                }
            }
        }
        if moved_any {
            sender.input(AppMsg::Refresh);
        }
    }

    /// Internal helper to populate the clipboard with the current selection.
    ///
    /// Args:
    ///     is_cut: If true, prefixes the text metadata with "cut" to signal a move operation.
    pub fn handle_clipboard_action(&self, is_cut: bool) {
        let selection = self.get_selection_with_meta();
        if selection.is_empty() {
            return;
        }

        let clipboard = gdk::Display::default().expect("No Display").clipboard();

        // Build the standard URI list (text/uri-list).
        // For archive:// virtual paths, extract to a temp location first so that
        // external apps receive a real file:// URI they can act on.
        let mut uri_list = String::new();
        for (path, is_dir) in &selection {
            let path_str = path.to_string_lossy();
            let uri = if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
                if let Some((archive_path, inner)) =
                    crate::services::archive::parse_archive_uri(&path_str)
                {
                    let cached_pwd = self.cached_archive_password.as_deref();
                    if *is_dir {
                        match crate::services::archive::extract_dir_to_tempdir(
                            &archive_path,
                            &inner,
                            cached_pwd,
                        ) {
                            Ok(tmp_dir) => gio::File::for_path(&tmp_dir).uri().to_string(),
                            Err(_) => continue,
                        }
                    } else {
                        match crate::services::archive::extract_entry_to_tempfile(
                            &archive_path,
                            &inner,
                            cached_pwd,
                        ) {
                            Ok(tmp) => {
                                let tmp_path = tmp.path().to_path_buf();
                                tmp.keep().ok();
                                gio::File::for_path(&tmp_path).uri().to_string()
                            }
                            Err(_) => continue,
                        }
                    }
                } else {
                    continue;
                }
            } else {
                gio::File::for_path(path).uri().to_string()
            };

            uri_list.push_str(&uri);
            uri_list.push_str("\r\n");
        }

        if uri_list.is_empty() {
            return;
        }

        // Build the Flux-internal metadata protocol
        let prefix = if is_cut { "cut" } else { "copy" };
        let mut text_rep = String::from(prefix);
        text_rep.push('\n');
        text_rep.push_str(&uri_list);

        // Create providers using raw bytes/strings
        // text/uri-list is the industry standard for file transfers
        let uri_provider = gdk::ContentProvider::for_bytes(
            "text/uri-list",
            &glib::Bytes::from(uri_list.as_bytes()),
        );

        // text/plain for our internal "cut" vs "copy" detection
        let text_provider = gdk::ContentProvider::for_value(&text_rep.to_value());

        // Combine in a Union
        let content = gdk::ContentProvider::new_union(&[uri_provider, text_provider]);

        clipboard.set_content(Some(&content)).unwrap();
    }

    /// Returns the canonical path to use as the folder cache key.
    /// If canonicalization fails, falls back to the given path.
    pub fn cache_key(&self, path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }
}
