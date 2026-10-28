use crate::model::{AppMsg, FluxApp, RightPanelType, TerminalPosition};
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
        crate::hit!("handle_terminal_cwd_changed");
        let path_str = self.current_path.to_string_lossy();

        let is_virtual = path_str.starts_with(crate::infra::archive::ARCHIVE_URI)
            || path_str.starts_with("trash://")
            || path_str.starts_with("recent://")
            || crate::infra::network::is_network_uri(&self.current_path);

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
        crate::hit!("handle_toggle_terminal");
        self.terminal_visible = !self.terminal_visible;

        let path_str = self.current_path.to_string_lossy();
        let effective_path = if path_str.starts_with(crate::infra::archive::ARCHIVE_URI) {
            crate::infra::archive::parse_archive_uri(&path_str)
                .map(|(archive_path, _)| {
                    archive_path
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "/".to_string())
                })
                .unwrap_or_else(|| "/".to_string())
        } else if crate::infra::network::is_network_uri(&self.current_path) {
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

    /// Working directory for a freshly spawned terminal: the archive's parent
    /// directory, `$HOME` on network locations, otherwise the current path.
    fn terminal_working_dir(&self) -> String {
        crate::hit!("terminal_working_dir");
        let path_str = self.current_path.to_string_lossy();
        if path_str.starts_with(crate::infra::archive::ARCHIVE_URI) {
            crate::infra::archive::parse_archive_uri(&path_str)
                .map(|(archive_path, _)| {
                    archive_path
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "/".to_string())
                })
                .unwrap_or_else(|| "/".to_string())
        } else if crate::infra::network::is_network_uri(&self.current_path) {
            env::var("HOME").unwrap_or_else(|_| "/".to_string())
        } else {
            self.current_path.to_str().unwrap_or("/").to_string()
        }
    }

    /// Enters or leaves the full-window terminal. The file-manager page of
    /// `main_stack` is swapped out, never destroyed, so leaving restores it as it was.
    ///
    /// The shell is never killed or respawned on leave: a running vim, htop or
    /// compile survives F4 out and F4 in. It is only killed on window close or
    /// application shutdown (see `register_terminal_shutdown`).
    pub fn handle_toggle_terminal_fullscreen(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_toggle_terminal_fullscreen");
        let Some(stack) = self.main_stack.clone() else {
            return;
        };

        if self.terminal_fullscreen {
            self.terminal_fullscreen = false;
            stack.set_visible_child_name("fm");

            if let Some(term) = &self.fullscreen_terminal {
                term.send_sigwinch();
            }
            // Mirror `toggle_sidebar_right_panel`'s close-focus: hand focus back to the grid.
            if let Some(tab) = self.tabs.get(self.active_tab_index) {
                tab.files.view.grab_focus();
            }
            return;
        }

        // Entering: nothing of the file manager stays visible, so stop its media.
        self.stop_video_preview();
        if self.preview_panel_visible {
            crate::ui::panels::preview_registry::with(|h| h.stop_media());
        }

        if self.fullscreen_terminal.is_none() {
            let Some(host) = self.fullscreen_terminal_box.clone() else {
                return;
            };

            let term = crate::infra::terminal::Terminal::new(&self.config.ui.terminal);
            crate::ui::startup::wire_terminal_theme(&term, &self.config.ui.terminal);

            // `capture.rs` and `navigation.rs` yield to any focused widget with this class.
            term.drawing_area.add_css_class("flux-terminal-area");
            term.drawing_area.set_hexpand(true);
            term.drawing_area.set_vexpand(true);

            // Wrap in a padded box so the prompt is not flush against the window
            // border or clipped by the rounded top corners in fullscreen mode.
            // The DrawingArea is measured by `spawn_async` for the PTY cols/rows,
            // so it must be sized to the inset rect, not the raw Stack page.
            let term_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
            term_box.set_margin_top(8);
            term_box.set_margin_start(8);
            term_box.set_margin_end(8);
            term_box.append(&term.drawing_area);
            host.append(&term_box);

            {
                let s = sender.clone();
                term.set_cwd_callback(move |path| {
                    s.input(AppMsg::TerminalCwdChanged(path));
                });
            }

            // Unlike the bottom pane (see `build_terminal`), F4 here must dispatch
            // `ToggleTerminalFullscreen` directly: we are already in the full-window
            // terminal, where F4 means "leave". The position cannot change while this
            // terminal is shown, because `SetTerminalPosition` closes it first.
            crate::ui::startup::attach_terminal_key_controller(
                &term,
                sender,
                AppMsg::ToggleTerminalFullscreen,
            );
            crate::ui::startup::attach_terminal_mouse_controllers(&term);
            crate::ui::startup::register_terminal_shutdown(&term, &stack);

            self.fullscreen_terminal = Some(term);
        }

        self.terminal_fullscreen = true;
        stack.set_visible_child_name("terminal");

        let Some(term) = self.fullscreen_terminal.clone() else {
            return;
        };

        if !self.terminal_spawned_fullscreen {
            // The spawn callback is `Send + 'static` and cannot touch `self`, so the
            // flag is set here, like `handle_toggle_terminal` does for the bottom pane.
            self.terminal_spawned_fullscreen = true;
            let working_dir = self.terminal_working_dir();

            // Spawn from an idle so the Stack page is already visible when the PTY is
            // sized. If it still has no allocation, `spawn_async` flags the PTY for an
            // initial SIGWINCH.
            glib::idle_add_local_once(move || {
                let mut term_spawn = term.clone();
                term_spawn.spawn_async(
                    0,
                    Some(&working_dir),
                    &[],
                    &[],
                    0,
                    || {},
                    -1,
                    None,
                    |result| {
                        if let Err(e) = result {
                            eprintln!("[flux] fullscreen terminal spawn: {e}");
                        }
                    },
                );
                glib::idle_add_local_once(move || {
                    term.grab_focus();
                    term.send_sigwinch();
                });
            });
        } else {
            term.send_sigwinch();
            glib::idle_add_local_once(move || {
                term.grab_focus();
            });
        }
    }

    /// Persists the terminal position. An open terminal is closed first, in whatever
    /// position it is in.
    pub fn handle_set_terminal_position(
        &mut self,
        pos: TerminalPosition,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_set_terminal_position");
        if pos == self.config.ui.terminal.position {
            return;
        }

        if self.terminal_fullscreen && pos != TerminalPosition::Fullscreen {
            self.handle_toggle_terminal_fullscreen(sender);
        }
        if self.terminal_panel_visible && pos != TerminalPosition::Sidebar {
            self.toggle_sidebar_right_panel(RightPanelType::Terminal, sender);
        }
        if self.terminal_visible && pos != TerminalPosition::Bottom {
            self.handle_toggle_terminal();
        }

        self.config.ui.terminal.position = pos;
        crate::utils::save_config(&self.config);
    }
}
