use crate::model::FluxApp;
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
        let path_str = path.to_string_lossy();

        // Determine if this is a network URI
        let file = if crate::services::network::is_network_uri(&path) {
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
            if crate::services::network::is_network_uri(&path) {
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
    pub fn remove_recents(paths: Option<&[PathBuf]>) -> std::io::Result<()> {
        let xbel_path = dirs::data_local_dir()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No data local dir"))?
            .join("recently-used.xbel");

        if !xbel_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&xbel_path)?;
        let lines: Vec<&str> = content.lines().collect();
        let mut keep = Vec::new();

        if let Some(paths) = paths {
            // Build a set of canonical file:// URIs to match against XBEL's href attributes.
            let uris_to_remove: HashSet<String> = paths
                .iter()
                .map(|p| gio::File::for_path(p).uri().into())
                .collect();

            for line in lines {
                if line.trim_start().starts_with("<bookmark") {
                    let should_remove =
                        uris_to_remove.iter().any(|uri| line.contains(uri.as_str()));
                    if !should_remove {
                        keep.push(line);
                    }
                } else {
                    keep.push(line);
                }
            }
        } else {
            // Remove all bookmarks: keep everything except `<bookmark …>` lines.
            for line in lines {
                if !line.trim_start().starts_with("<bookmark") {
                    keep.push(line);
                }
            }
        }

        std::fs::write(&xbel_path, keep.join("\n"))?;
        Ok(())
    }
}
