use crate::model::FluxApp;
use gtk::glib;
use gtk::prelude::*;
use std::env;

impl FluxApp {
    /// Handles toggling the embedded terminal panel's visibility, PTY spawning, and geometry sync.
    pub fn handle_toggle_terminal(&mut self) {
        self.terminal_visible = !self.terminal_visible;

        if self.terminal_visible {
            if !self.terminal_cleared {
                self.terminal_cleared = true;
            }

            if !self.terminal_spawned {
                self.terminal_spawned = true;

                let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
                let startup_path = self.current_path.to_str().unwrap_or("/").to_string();
                let mut term_clone = self.terminal.clone();

                term_clone.spawn_async(
                    0,
                    Some(&startup_path),
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
            } else if let Some(dir) = self.current_path.to_str() {
                self.terminal.respawn(dir);
            }

            // Set the paned position using char_height from the terminal state so fish starts with the correct row count
            if let Some(paned) = &self.terminal_paned {
                let height = paned.height();
                if height > 0 {
                    let char_height = self.terminal.char_height().max(1);
                    let terminal_height = self.config.ui.terminal.height * char_height;
                    paned.set_position(height - terminal_height);
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
