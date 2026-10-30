use crate::domain::model::{AppMsg, FluxApp};
use crate::utils::path::PathExt;
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::process::Command;

impl FluxApp {
    pub fn expand_path(path: &str) -> PathBuf {
        // We delegate the logic to our PathExt trait which handles
        // component stripping and home directory joining safely.

        PathBuf::from(path).expand_tilde()
    }

    pub fn open_file(path: PathBuf) {
        crate::hit!("open_file");
        let path_str = path.to_string_lossy();

        // Determine if this is a network URI
        let file = if crate::infra::network::is_network_uri(&path) {
            gio::File::for_uri(&path_str)
        } else {
            gio::File::for_path(&path)
        };

        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        let (content_type, _) = gio::content_type_guess(Some(filename.as_ref()), None);
        if let Some(app_info) = gio::AppInfo::default_for_type(&content_type, false) {
            let _ = app_info.launch(&[file], None::<&gio::AppLaunchContext>);
        } else {
            // Fallback: if we have a URI, use xdg-open with the URI, else the path
            if crate::infra::network::is_network_uri(&path) {
                let _ = Command::new("xdg-open").arg(&*path_str).spawn();
            } else {
                let _ = Command::new("xdg-open").arg(path).spawn();
            }
        }
    }

    /// Removes recent entries from the `~/.local/share/recently-used.xbel` file.
    ///
    /// If `paths` is `None`, all bookmarks are removed.
    /// If `Some(paths)`, only bookmarks whose `href` matches one of the given paths are removed.
    ///
    /// # Errors
    /// Returns an I/O error if the XBEL file cannot be read or written.
    pub fn remove_recents(paths: Option<Vec<PathBuf>>) -> std::io::Result<()> {
        crate::hit!("remove_recents");
        let xbel_path = dirs::data_local_dir()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No data local dir"))?
            .join("recently-used.xbel");

        if !xbel_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&xbel_path)?;
        let hrefs: Option<HashSet<String>> = paths.map(|paths| {
            paths
                .iter()
                .map(|p| format!("href=\"{}\"", gio::File::for_path(p).uri()))
                .collect()
        });

        let mut keep: Vec<&str> = Vec::new();
        let mut skipping = false;
        for line in content.lines() {
            let trimmed = line.trim_start();
            if skipping {
                skipping = !trimmed.starts_with("</bookmark>");
                continue;
            }
            if trimmed.starts_with("<bookmark ")
                && hrefs
                    .as_ref()
                    .is_none_or(|set| set.iter().any(|h| line.contains(h.as_str())))
            {
                skipping = !trimmed.contains("</bookmark>") && !trimmed.ends_with("/>");
                continue;
            }
            keep.push(line);
        }

        let tmp = xbel_path.with_extension("xbel.tmp");
        std::fs::write(&tmp, keep.join("\n") + "\n")?;
        std::fs::rename(&tmp, &xbel_path)
    }

    /// Applies the outcome of the background `remove_recents` run started by
    /// `handle_clear_recents`: refreshes the view on success, toasts on failure.
    pub fn handle_clear_recents_finished(
        &mut self,
        result: Result<(), String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_clear_recents_finished");
        match result {
            Ok(()) => {
                self.recents_has_selection = false;
                sender.input(AppMsg::Refresh);
            }
            Err(e) => {
                sender.input(AppMsg::ShowToast(format!("Failed to clear recents: {}", e)));
            }
        }
    }
}
