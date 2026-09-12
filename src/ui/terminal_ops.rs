use crate::model::{AppMsg, FluxApp};
use gtk::glib;
use gtk::prelude::*;
use relm4::AsyncComponentSender;
use std::env;
use std::path::PathBuf;

impl FluxApp {
    /// Handles directory sync events from the terminal's OSC 7 escape sequence.
    pub fn handle_terminal_cwd_changed(
        &mut self,
        path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let path_str = self.current_path.to_string_lossy();

        // Do not let terminal directory changes pull the GUI out of virtual locations
        if path_str.starts_with(crate::services::archive::ARCHIVE_URI)
            || path_str.starts_with("trash://")
            || path_str.starts_with("recent://")
            || crate::services::network::is_network_uri(&self.current_path)
        {
            return;
        }

        if self.current_path != path {
            sender.input(AppMsg::Navigate(path));
        }
    }

    /// Handles toggling the embedded terminal panel's visibility, PTY spawning, and geometry sync.
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
                self.terminal.respawn(&effective_path);
            }

            // Set the paned position using char_height from the terminal state so fish starts with the correct row count
            if let Some(paned) = &self.terminal_paned {
                let height = paned.height();
                if height > 0 {
                    paned.set_position(height - self.config.ui.terminal.height);
                }
            }

            let term = self.terminal.clone();
            glib::idle_add_local_once(move || {
                term.grab_focus();
                // Send SIGWINCH after pane layout settles so shell re-reads $LINES/$COLUMNS
                term.send_sigwinch();
            });
        } else {
            // Hide terminal: terminate the active shell process and reset flags
            self.terminal.kill_shell();
            self.terminal_spawned = false;
            self.terminal_cleared = false;
        }
    }
}
