//! File actions: item activation, moves and clipboard export.

use crate::app::state::FluxApp;
use crate::app::update::file_ops::paste::{is_same_entry, paste_name_for};
use crate::domain::model::{
    AppMsg, ArchiveMsg, ConflictPolicy, DeviceMsg, NavigationMsg, WindowMsg,
};
use crate::i18n::tr;
use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use relm4::prelude::*;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

/// AppImage crashes this soon after spawn are treated as startup failures.
const APPIMAGE_STARTUP_WINDOW: Duration = Duration::from_secs(10);

/// Known "wrong display backend" startup failures, as
/// `(stderr needle, env var, value)`. When an AppImage dies at startup with
/// the needle on stderr, it is relaunched once with `env var = value`.
const APPIMAGE_BACKEND_FALLBACKS: &[(&str, &str, &str)] = &[(
    "Could not find the Qt platform plugin",
    "QT_QPA_PLATFORM",
    "xcb",
)];

/// Spawns an AppImage with stderr captured to an anonymous temp file.
///
/// A temp file is used instead of a pipe: if flux exits while the app is still
/// running, a closed pipe would hand the app EPIPE/SIGPIPE on its next write.
fn spawn_appimage(
    path: &Path,
    workdir: &Path,
    extra_env: Option<(&str, &str)>,
) -> std::io::Result<(Child, fs::File)> {
    let log = tempfile::tempfile()?;
    let mut cmd = std::process::Command::new(path);
    cmd.current_dir(workdir)
        .stderr(Stdio::from(log.try_clone()?));
    if let Some((key, value)) = extra_env {
        cmd.env(key, value);
    }
    Ok((cmd.spawn()?, log))
}

impl FluxApp {
    /// Returns true if the current path is inside an archive that supports full extraction.
    pub fn can_extract_current_archive(&self) -> bool {
        crate::hit!("can_extract_current_archive");
        let path_str = self.current_path.to_string_lossy();
        if !path_str.starts_with(crate::infra::archive::ARCHIVE_URI) {
            return false;
        }
        crate::infra::archive::parse_archive_uri(&path_str)
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

    /// Launches an AppImage as a native executable.
    ///
    /// If the file lacks the owner-execute bit, which is what downloaded
    /// AppImages usually have, we set it silently and spawn. This matches the
    /// macOS `.app` bundle behavior that AppImage is explicitly imitating:
    /// double-click runs the app, no confirmation dialog. On Linux this is a
    /// deliberate departure from file managers convention of prompting
    /// on first run, the reasoning is that an AppImage is understood by users
    /// to be a runnable artifact, and the prompt adds friction without
    /// meaningful protection (the user already chose to double-click it).
    ///
    /// The chmod is owner-only (`0o100`), never touches group/other bits,
    /// setuid, setgid, or ACLs, and is trivially reversible with `chmod -x`.
    fn launch_appimage(path: &Path) -> std::io::Result<()> {
        crate::hit!("launch_appimage");
        use std::os::unix::fs::PermissionsExt;

        // Ensure the owner-execute bit is set. AppImages downloaded via a browser
        // usually arrive mode 0644, which is why double-clicking them in other
        // file managers historically does nothing.
        let meta = std::fs::metadata(path)?;
        let mut perms = meta.permissions();
        let mode = perms.mode();
        if mode & 0o100 == 0 {
            perms.set_mode(mode | 0o100);
            std::fs::set_permissions(path, perms)?;
            eprintln!("[appimage] granted +x to {}", path.display());
        }

        let workdir = path
            .parent()
            .unwrap_or_else(|| Path::new("/"))
            .to_path_buf();
        let (mut child, mut log) = spawn_appimage(path, &workdir, None)?;
        let path = path.to_path_buf();

        std::thread::spawn(move || {
            let started = Instant::now();
            let Ok(status) = child.wait() else { return };

            let mut captured = String::new();
            if log.seek(SeekFrom::Start(0)).is_ok() {
                let mut buf = Vec::new();
                let _ = Read::take(&mut log, 64 * 1024).read_to_end(&mut buf);
                captured = String::from_utf8_lossy(&buf).into_owned();
            }
            let _ = std::io::stderr().write_all(captured.as_bytes());

            if status.success() || started.elapsed() > APPIMAGE_STARTUP_WINDOW {
                return;
            }
            for (needle, key, value) in APPIMAGE_BACKEND_FALLBACKS {
                let already_set = std::env::var(key).is_ok_and(|v| v == *value);
                if captured.contains(needle) && !already_set {
                    eprintln!("[appimage] retrying {} with {key}={value}", path.display());
                    let _ = spawn_appimage(&path, &workdir, Some((key, value)));
                    return;
                }
            }
        });
        Ok(())
    }

    /// Dispatches the correct action for each `(path, is_dir)` item pair.
    ///
    /// Centralises the routing logic shared by `NavigationMsg::Open` and `NavigationMsg::Activate`
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
        crate::hit!("activate_items");
        #[allow(clippy::never_loop)]
        for (path, is_dir) in items {
            let path_str = path.to_string_lossy();

            if let Some((archive_path, inner)) = crate::infra::archive::parse_archive_uri(&path_str)
            {
                if is_dir {
                    sender.input(AppMsg::Navigation(NavigationMsg::Navigate(path)));
                } else if crate::infra::archive::is_browsable_archive(Path::new(&inner)) {
                    sender.input(AppMsg::Archive(ArchiveMsg::EnterArchive(path)));
                } else {
                    let sender_clone = sender.clone();
                    let cached_pwd = self.archive.passwords.get(&archive_path).cloned();

                    let parent_prefix = Path::new(&inner)
                        .parent()
                        .and_then(|p| p.to_str())
                        .unwrap_or("")
                        .to_string();

                    relm4::spawn_blocking(move || {
                        match crate::infra::archive::extract_entry_to_tempfile(
                            &archive_path,
                            &inner,
                            cached_pwd.as_deref(),
                        ) {
                            Ok(tmp) => {
                                if let Ok((_file, path_buf)) = tmp.keep() {
                                    Self::open_file(path_buf);
                                }
                            }
                            Err(crate::domain::model::ArchiveError::PasswordRequired) => {
                                sender_clone.input(AppMsg::Archive(
                                    ArchiveMsg::PromptArchivePassword {
                                        archive_path,
                                        prefix: parent_prefix,
                                        wrong_password: false,
                                    },
                                ));
                            }
                            Err(crate::domain::model::ArchiveError::WrongPassword) => {
                                sender_clone.input(AppMsg::Archive(
                                    ArchiveMsg::PromptArchivePassword {
                                        archive_path,
                                        prefix: parent_prefix,
                                        wrong_password: true,
                                    },
                                ));
                            }
                            Err(e) => {
                                sender_clone
                                    .input(AppMsg::Window(WindowMsg::ShowToast(e.to_string())));
                            }
                        }
                    });
                }
                break;
            } else if is_dir {
                sender.input(AppMsg::Navigation(NavigationMsg::Navigate(path)));
                break;
            } else if crate::infra::archive::is_browsable_archive(&path) {
                sender.input(AppMsg::Archive(ArchiveMsg::EnterArchive(path)));
                break;
            }
            // LUKS encrypted image, AppImage, .desktop entry, or standard file open
            else {
                let sender_clone = sender.clone();
                let path_clone = path.clone();
                relm4::spawn_blocking(move || {
                    if crate::infra::luks::is_luks_image(&path_clone) {
                        sender_clone.input(AppMsg::Device(DeviceMsg::UnlockLuksImage {
                            path: path_clone,
                        }));
                    } else if crate::utils::media::is_appimage(&path_clone) {
                        if let Err(e) = Self::launch_appimage(&path_clone) {
                            let msg =
                                tr("Failed to launch AppImage: {}").replace("{}", &e.to_string());
                            sender_clone.input(AppMsg::Window(WindowMsg::ShowToast(msg)));
                        }
                    } else if path_clone.extension().and_then(|e| e.to_str()) == Some("desktop") {
                        if let Some(desktop_app) =
                            gio_unix::DesktopAppInfo::from_filename(&path_clone)
                        {
                            let app_name = desktop_app.display_name().to_string();
                            let context = gdk::Display::default().map(|d| d.app_launch_context());
                            match desktop_app.launch(&[], context.as_ref()) {
                                Ok(_) => {
                                    let msg = tr("Opening {}").replace("{}", &app_name);
                                    sender_clone.input(AppMsg::Window(WindowMsg::ShowToast(msg)));
                                }
                                Err(e) => {
                                    eprintln!("[flux] Failed to launch desktop file: {e}");
                                    let msg = tr("Failed to open {}: {}")
                                        .replacen("{}", &app_name, 1)
                                        .replacen("{}", &e.to_string(), 1);
                                    sender_clone.input(AppMsg::Window(WindowMsg::ShowToast(msg)));
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
        crate::hit!("handle_move_files_to_target");
        let mut skipped_dirs = false;
        let resolved: Vec<(PathBuf, String, bool)> = sources
            .into_iter()
            .filter_map(|src| {
                if src == destination {
                    return None;
                }
                let orig_name = src.file_name()?.to_string_lossy().into_owned();
                let name = paste_name_for(&src, &orig_name);
                let dest_path = destination.join(&name);
                if is_same_entry(&src, &dest_path) {
                    return None;
                }
                let is_dir = src.symlink_metadata().map(|m| m.is_dir()).unwrap_or(false);
                if is_dir && dest_path.symlink_metadata().is_ok() {
                    skipped_dirs = true;
                    return None;
                }
                Some((src, name, is_dir))
            })
            .collect();

        if skipped_dirs {
            sender.input(AppMsg::Window(WindowMsg::ShowToast(tr(
                "Skipped folders that already exist in the destination",
            ))));
        }

        self.run_paste_batch(
            resolved,
            true,
            destination,
            sender.clone(),
            ConflictPolicy::Ask,
        );
    }

    /// Internal helper to populate the clipboard with the current selection.
    ///
    /// Args:
    ///     is_cut: If true, prefixes the text metadata with "cut" to signal a move operation.
    pub fn handle_clipboard_action(&self, is_cut: bool) {
        crate::hit!("handle_clipboard_action");
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
            let uri = if path_str.starts_with(crate::infra::archive::ARCHIVE_URI) {
                if let Some((archive_path, inner)) =
                    crate::infra::archive::parse_archive_uri(&path_str)
                {
                    let cached_pwd = self
                        .archive
                        .passwords
                        .get(&archive_path)
                        .map(String::as_str);
                    if *is_dir {
                        match crate::infra::archive::extract_dir_to_tempdir(
                            &archive_path,
                            &inner,
                            cached_pwd,
                        ) {
                            Ok(tmp_dir) => gio::File::for_path(&tmp_dir).uri().to_string(),
                            Err(_) => continue,
                        }
                    } else {
                        match crate::infra::archive::extract_entry_to_tempfile(
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

        let content = gdk::ContentProvider::new_union(&[uri_provider, text_provider]);

        clipboard.set_content(Some(&content)).unwrap();
    }

    /// Returns the canonical path to use as the folder cache key.
    /// If canonicalization fails, falls back to the given path.
    pub fn cache_key(&self, path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    /// Evicts the folder-cache entries invalidated by a change to `path`.
    pub fn evict_folder_cache(&mut self, path: &Path) {
        if let Some(parent) = path.parent() {
            self.folder_cache.remove(&self.cache_key(parent));
            if let Some(name) = path.file_name() {
                self.folder_cache.remove(&self.cache_key(parent).join(name));
            }
        }
        self.folder_cache.remove(&self.cache_key(path));
    }
}
