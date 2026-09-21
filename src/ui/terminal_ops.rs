use crate::model::{AppMsg, FluxApp};
use gtk::glib;
use gtk::prelude::*;
use relm4::AsyncComponentSender;
use std::env;
use std::path::PathBuf;

impl FluxApp {
    pub fn handle_terminal_cwd_changed(
        &mut self,
        path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let path_str = self.current_path.to_string_lossy();

        let is_virtual = path_str.starts_with(crate::services::archive::ARCHIVE_URI)
            || path_str.starts_with("trash://")
            || path_str.starts_with("recent://")
            || crate::services::network::is_network_uri(&self.current_path);

        if !is_virtual {
            let curr_canon = self
                .current_path
                .canonicalize()
                .unwrap_or_else(|_| self.current_path.clone());
            let new_canon = path.canonicalize().unwrap_or_else(|_| path.clone());

            if curr_canon != new_canon {
                sender.input(AppMsg::Navigate(path));
            }
        }
    }

    pub fn handle_toggle_terminal(&mut self) {
        self.terminal_visible = !self.terminal_visible;

        let path_str = self.current_path.to_string_lossy();
        let effective_path = if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
            crate::services::archive::parse_archive_uri(&path_str)
                .map(|(archive_path, _)| {
                    archive_path
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "/".to_string())
                })
                .unwrap_or_else(|| "/".to_string())
        } else if crate::services::network::is_network_uri(&self.current_path) {
            env::var("HOME").unwrap_or_else(|_| "/".to_string())
        } else {
            self.current_path.to_str().unwrap_or("/").to_string()
        };

        if self.terminal_visible {
            if !self.terminal_cleared {
                self.terminal_cleared = true;
            }

            if let Some(paned) = &self.terminal_paned {
                let paned_clone = paned.clone();
                let target_h = self.config.ui.terminal.height;

                glib::idle_add_local_once(move || {
                    let total_h = paned_clone.height();
                    if total_h > target_h && target_h > 0 {
                        paned_clone.set_position(total_h - target_h);
                    }
                });

                paned.connect_position_notify(|p| {
                    let total_h = p.height();
                    let pos = p.position();
                    if total_h > 50 && pos > 0 && pos < total_h {
                        let pixel_h = total_h - pos;
                        let mut cfg = crate::utils::load_config();
                        if cfg.ui.terminal.height != pixel_h {
                            cfg.ui.terminal.height = pixel_h;
                            crate::utils::save_config(&cfg);
                        }
                    }
                });
            }

            if !self.terminal_spawned {
                self.terminal_spawned = true;

                let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
                let mut term_clone = self.terminal.clone();

                term_clone.spawn_async(
                    0,
                    Some(&effective_path),
                    &[&shell],
                    &[],
                    0,
                    || {},
                    -1,
                    None,
                    move |result| {
                        if let Err(e) = result {
                            eprintln!("Failed to spawn shell: {}", e);
                        }
                    },
                );
            } else {
                let is_idle = self
                    .terminal
                    .state
                    .lock()
                    .map(|s| s.is_idle())
                    .unwrap_or(false);

                if is_idle {
                    self.terminal.respawn(&effective_path);
                }
            }

            let term = self.terminal.clone();
            glib::idle_add_local_once(move || {
                term.grab_focus();
                term.send_sigwinch();
            });
        }
    }
}
