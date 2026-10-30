use crate::domain::model::{AppMsg, FluxApp};
use crate::utils;
use relm4::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

impl FluxApp {
    pub fn rename_path(old_path: &Path, new_name: &str) -> std::io::Result<PathBuf> {
        crate::hit!("rename_path");
        // Reject any name containing a path separator to prevent directory traversal
        if new_name.contains(std::path::MAIN_SEPARATOR) || new_name.contains('/') {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "new name must be a plain filename, not a path",
            ));
        }

        let mut new_path = old_path.to_path_buf();
        new_path.set_file_name(new_name);

        if new_path.symlink_metadata().is_ok() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "A file with this name already exists",
            ));
        }

        fs::rename(old_path, &new_path)?;
        Ok(new_path)
    }

    /// Handles file and directory renames with error handling for permissions.
    pub fn handle_perform_rename(
        &mut self,
        old_path: PathBuf,
        new_name: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_perform_rename");
        match Self::rename_path(&old_path, &new_name) {
            Ok(new_path) => {
                let _ = self.state_db.rename_path(&old_path, &new_path);

                let old_key = old_path.to_string_lossy().to_string();
                let new_key = new_path.to_string_lossy().to_string();

                crate::infra::db::rekey_path_prefix(
                    &mut self.config.ui.file_icons,
                    &old_key,
                    &new_key,
                );
                crate::infra::db::rekey_path_prefix(
                    &mut self.config.ui.folder_icons,
                    &old_key,
                    &new_key,
                );

                utils::save_config(&self.config);

                if let Some(parent) = old_path.parent() {
                    self.folder_cache.remove(&self.cache_key(parent));
                }
                self.folder_cache
                    .remove(&self.cache_key(&self.current_path));

                let canon_parent = old_path.parent().map(|p| self.cache_key(p));
                let canon_join = |p: &Path| match (&canon_parent, p.file_name()) {
                    (Some(dir), Some(name)) => dir.join(name),
                    _ => p.to_path_buf(),
                };
                let canon_old = canon_join(&old_path);
                let canon_new = canon_join(&new_path);
                self.file_op_history
                    .push_undo(crate::ui::undo_redo::FileOp::Rename {
                        old_path: canon_old,
                        new_path: canon_new,
                        old_name: old_path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        new_name: new_name.clone(),
                    });

                sender.input(AppMsg::Navigate(self.current_path.clone()));
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Permission denied") || msg.contains("Operation not permitted") {
                    sender.input(AppMsg::ShowToast(
                        "Permission denied: Cannot rename item.".into(),
                    ));
                } else {
                    sender.input(AppMsg::ShowToast(format!("Rename error: {}", e)));
                }
            }
        }
    }
}
