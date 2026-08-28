use crate::model::{AppMsg, FluxApp};
use crate::ui::SidebarPlace;
use crate::utils;
use adw::gio;
use adw::gio::prelude::*;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Reloads sidebar places from config and appends active mounts.
    pub fn handle_refresh_sidebar(&mut self) {
        self.config = utils::load_config();
        self.sidebar.guard().clear();
        for place in &self.config.sidebar {
            let is_label = place.kind.as_deref() == Some("label");
            let path = if is_label {
                PathBuf::new()
            } else if place.path == "tags://" {
                PathBuf::from(&place.path)
            } else {
                utils::expand_path(&place.path)
            };

            self.sidebar.guard().push_back(crate::ui::SidebarPlace {
                name: place.name.clone(),
                icon: place.icon.clone(),
                path,
                is_mount: false,
                is_section_label: is_label,
            });
        }
        self.refresh_sidebar();
    }

    pub fn handle_system_mounts_ready(&mut self, mounts: Vec<(String, std::path::PathBuf)>) {
        let mut guard = self.sidebar.guard();

        // Remove existing mount items from the back to maintain index stability
        let mut idx = guard.len();
        while idx > 0 {
            idx -= 1;
            if guard.get(idx).map(|p| p.is_mount).unwrap_or(false) {
                guard.remove(idx);
            }
        }

        let home = dirs::home_dir().unwrap_or_default();
        let home_str = home.to_string_lossy().to_string();

        for (mut name, path) in mounts {
            let path_str = path.to_string_lossy().to_string();
            let trimmed_path = path_str.trim_end_matches('/');
            let mut icon = "drive-harddisk-symbolic".to_string();

            let rename_opt = self
                .config
                .ui
                .device_renames
                .get(&path_str)
                .or_else(|| self.config.ui.device_renames.get(trimmed_path))
                .or_else(|| {
                    self.config.ui.device_renames.iter().find_map(|(k, v)| {
                        let expanded = if k.starts_with('~') {
                            k.replace('~', &home_str)
                        } else {
                            k.clone()
                        };
                        let exp_trimmed = expanded.trim_end_matches('/');
                        if exp_trimmed == trimmed_path || exp_trimmed == path_str || k == &name {
                            Some(v)
                        } else {
                            None
                        }
                    })
                });

            if let Some(rename) = rename_opt {
                name = rename.name.clone();
                if let Some(custom_icon) = &rename.icon {
                    icon = custom_icon.clone();
                }
            } else if name.to_lowercase().contains("drive")
                || name.to_lowercase().contains("cloud")
                || path_str.contains("Gdrive")
            {
                icon = "folder-remote-symbolic".to_string();
            }

            guard.push_back(SidebarPlace {
                name,
                icon,
                path,
                is_mount: true,
                is_section_label: false,
            });
        }
    }

    /// Removes a custom location from the sidebar configuration.
    pub fn handle_remove_from_sidebar(&mut self, path: PathBuf) {
        let path_str = path.to_string_lossy();
        let home = dirs::home_dir().unwrap_or_default();
        self.config.sidebar.retain(|entry| {
            let expanded = if entry.path.starts_with('~') {
                entry.path.replacen('~', &home.to_string_lossy(), 1)
            } else {
                entry.path.clone()
            };
            expanded != path_str.as_ref()
        });
        utils::save_config(&self.config);
        self.refresh_sidebar();
    }

    /// Toggles permanent pinning of the currently selected or active directory.
    pub fn handle_add_to_sidebar_permanent(&mut self) {
        let path = self
            .get_selected_path()
            .unwrap_or_else(|| self.current_path.clone());
        let expanded_path = crate::utils::expand_path(&path.to_string_lossy());
        let path_str = expanded_path.to_string_lossy().to_string();

        let existing_idx = self.config.sidebar.iter().position(|entry| {
            let expanded = crate::utils::expand_path(&entry.path);
            expanded.to_string_lossy() == path_str
        });

        if let Some(idx) = existing_idx {
            self.config.sidebar.remove(idx);
        } else {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path_str.clone());
            self.config.sidebar.insert(
                0,
                crate::model::CustomPlace {
                    name,
                    kind: None,
                    icon: "folder-symbolic".to_string(),
                    path: path_str,
                },
            );
        }
        utils::save_config(&self.config);
        self.refresh_sidebar();
    }

    /// Reorders custom entries in the sidebar bookmark list.
    pub fn handle_reorder_sidebar(&mut self, from: PathBuf, to: PathBuf) {
        let home = dirs::home_dir().unwrap_or_default();
        let home_str = home.to_string_lossy().to_string();

        let resolve_key = |place: &crate::model::CustomPlace| -> String {
            if place.kind.as_deref() == Some("label") {
                format!("label:{}", place.name)
            } else if place.path.starts_with('~') {
                place.path.replacen('~', &home_str, 1)
            } else {
                place.path.clone()
            }
        };

        let from_str = from.to_string_lossy().to_string();
        let to_str = to.to_string_lossy().to_string();

        let from_idx = self
            .config
            .sidebar
            .iter()
            .position(|e| resolve_key(e) == from_str);

        let to_idx = self
            .config
            .sidebar
            .iter()
            .position(|e| resolve_key(e) == to_str);

        if let (Some(fi), Some(ti)) = (from_idx, to_idx) {
            let entry = self.config.sidebar.remove(fi);
            let insert_at = ti;
            self.config.sidebar.insert(insert_at, entry);

            crate::utils::save_config(&self.config);
            self.refresh_sidebar();
        }
    }

    /// Pins a dropped folder directly before a target row in the sidebar.
    pub fn handle_pin_folder_at(
        &mut self,
        path: PathBuf,
        before: PathBuf,
        label_name: Option<String>,
    ) {
        let home = dirs::home_dir().unwrap_or_default();
        let home_str = home.to_string_lossy();
        let path_str = path.to_string_lossy().to_string();
        let before_str = before.to_string_lossy().to_string();

        let resolve = |entry_path: &str| -> String {
            if entry_path.starts_with('~') {
                entry_path.replacen('~', &home_str, 1)
            } else {
                entry_path.to_owned()
            }
        };

        let already = self
            .config
            .sidebar
            .iter()
            .any(|e| resolve(&e.path) == path_str);
        if already {
            return;
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path_str.clone());

        let new_entry = crate::model::CustomPlace {
            name,
            kind: None,
            icon: "folder-symbolic".to_string(),
            path: path_str,
        };

        let insert_at = if let Some(label_name) = label_name {
            // Look for a label entry with matching name
            self.config
                .sidebar
                .iter()
                .position(|e| e.kind.as_deref() == Some("label") && e.name == label_name)
                .unwrap_or(self.config.sidebar.len())
        } else {
            // Fallback to path matching for non-label rows
            self.config
                .sidebar
                .iter()
                .position(|e| resolve(&e.path) == before_str)
                .unwrap_or(self.config.sidebar.len())
        };

        self.config.sidebar.insert(insert_at, new_entry);
        utils::save_config(&self.config);
        self.refresh_sidebar();
    }

    /// Moves files dragged from the grid into a sidebar folder destination.
    ///
    /// Mirrors `handle_drop_items` but originates from the sidebar drop zone instead of
    /// a grid cell. Only items whose source path differs from the computed destination are
    /// moved, same-path no-ops are silently skipped. After all moves the view is refreshed.
    pub fn handle_sidebar_drop_move(
        &self,
        source_paths: Vec<PathBuf>,
        dest_path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let sender_clone = sender.clone();

        relm4::spawn_blocking(move || {
            for source_path in source_paths {
                if !dest_path.is_dir() {
                    break;
                }

                let Some(file_name) = source_path.file_name() else {
                    continue;
                };

                let final_dest = dest_path.join(file_name);
                if source_path == final_dest {
                    continue;
                }

                let src_file = gio::File::for_path(&source_path);
                let dst_file = gio::File::for_path(&final_dest);

                if src_file
                    .move_(
                        &dst_file,
                        gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                        gio::Cancellable::NONE,
                        None,
                    )
                    .is_ok()
                {
                    sender_clone.input(AppMsg::ItemMoved {
                        old_path: source_path,
                        new_path: final_dest,
                    });
                } else {
                    eprintln!(
                        "[Sidebar DnD] Failed to move {:?} → {:?}",
                        source_path, dest_path
                    );
                }
            }

            sender_clone.input(AppMsg::Refresh);
        });
    }

    pub fn handle_toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
        self.config.ui.sidebar_visible = self.sidebar_visible;
        crate::utils::save_config(&self.config);
        if let Some(ref widget) = self.sidebar_widget {
            widget.set_visible(self.sidebar_visible);
        }
    }

    /// Unmounts a mounted storage volume or external drive.
    pub fn handle_unmount_device(&self, path: PathBuf, sender: &AsyncComponentSender<Self>) {
        let sender = sender.clone();
        let file = gio::File::for_path(&path);

        if let Ok(mount) = file.find_enclosing_mount(gio::Cancellable::NONE) {
            mount.unmount_with_operation(
                gio::MountUnmountFlags::NONE,
                gio::MountOperation::NONE,
                gio::Cancellable::NONE,
                move |res| match res {
                    Ok(_) => sender.input(AppMsg::RefreshSidebar),
                    Err(e) => sender.input(AppMsg::ShowToast(format!("Unmount failed: {}", e))),
                },
            );
        }
    }
}
