//! Shell process spawning over a PTY.

use super::draw::cell_size;
use super::handler::TerminalHandler;
use super::Terminal;
use gtk::gio;
use gtk::prelude::*;
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use vte::Parser;

impl Terminal {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_async<F>(
        &mut self,
        _pty_flags: u32,
        working_dir: Option<&str>,
        _argv: &[&str],
        _envv: &[&str],
        _spawn_flags: u32,
        _child_setup: F,
        _timeout: i32,
        _cancellable: Option<&gio::Cancellable>,
        callback: impl FnOnce(Result<glib::Pid, glib::Error>) + 'static + Send,
    ) where
        F: Fn() + 'static + Send,
    {
        crate::hit!("spawn_async");
        // Ensure `theme_env` is populated before the child is forked, so the
        // `FLUX_THEME_*` exports below reach the shell via `execve`.
        //
        // The bottom terminal has a `connect_map` -> `apply_theme` hook (see
        // `ui/startup/terminal.rs`), so on its first spawn `theme_env` may
        // already be set and this is a no-op. The sidebar terminal is built
        // by `panels/terminal.rs` without that hook, so without this guard
        // its `theme_env` would stay `None` forever and the fish snippet
        // would never activate. `apply_theme` is idempotent and cheap (a
        // handful of `lookup_color` calls plus palette derivation), and
        // `write_fish_theme_snippet` spawns its own writer thread, so this
        // does not block the spawn path.
        if self.theme_env.lock().map(|g| g.is_none()).unwrap_or(true) {
            self.apply_theme(&self.config);
        }

        let working_dir = working_dir.map(str::to_owned);

        let (width, height) = (self.drawing_area.width(), self.drawing_area.height());

        let layout = self.drawing_area.create_pango_layout(None);
        layout.set_font_description(Some(
            &self
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .font_desc,
        ));
        let (char_width, char_height) = cell_size(&layout);

        let cols = if width > 0 {
            (width as f64 / char_width).floor() as usize
        } else {
            80
        };
        let rows = if height > 0 {
            (height as f64 / char_height).floor() as usize
        } else {
            24
        };
        let spawned_hidden = width == 0 || height == 0;

        let (master_fd, slave_fd): (RawFd, RawFd) = unsafe {
            let mut master: RawFd = -1;
            let mut slave: RawFd = -1;
            let winsize = libc::winsize {
                ws_row: rows as u16,
                ws_col: cols as u16,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            if libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &winsize,
            ) != 0
            {
                callback(Err(glib::Error::new(
                    glib::FileError::Failed,
                    "Failed to open PTY",
                )));
                return;
            }
            (master, slave)
        };

        // Disable echo on the master side only, the shell handles its own
        // echoing. Preserving ONLCR (NL→CR+NL translation) is intentional so
        // bare \n from the shell still moves the cursor to column 0.
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(master_fd, &mut termios) == 0 {
                termios.c_lflag &= !(libc::ECHO | libc::ECHOE | libc::ECHOK | libc::ECHONL);
                libc::tcsetattr(master_fd, libc::TCSANOW, &termios);
            }
        };

        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.cleaned_up = false;
            state.pty_master_fd = Some(master_fd);
            state.cols = cols;
            state.rows = rows;
            state.needs_initial_sigwinch = spawned_hidden;
        }

        let target_shell = self
            .config
            .shell
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(String::from)
            .unwrap_or_else(|| {
                if std::path::Path::new("/app/bin/fish").exists() {
                    return "/app/bin/fish".to_string();
                }

                if let Ok(env_shell) = std::env::var("SHELL") {
                    if !env_shell.trim().is_empty() && std::path::Path::new(&env_shell).exists() {
                        return env_shell;
                    }
                }

                "/bin/bash".to_string()
            });

        // Spawn the shell directly on the PTY slave
        let mut command = Command::new(&target_shell);
        command.arg("-l");
        command.env("TERM", "xterm-256color");
        command.env("FLUX_PID", std::process::id().to_string());
        command.env_remove("LD_LIBRARY_PATH");
        command.env_remove("LD_PRELOAD");

        // ── Theme Injection ──────────────────────────────────────────────
        // Export the resolved GTK theme colours so the fish `conf.d` snippet
        // (written by `fish_theme::write_fish_theme_snippet`) can map them to
        // `fish_color_*` language variables.
        //
        // Gated on `ui.fish_theme` so the setting actually does what it says:
        // with it off, no `FLUX_THEME_*` reaches the child, and the snippet's
        // own `if set -q FLUX_THEME_FG` guard keeps any stale file on disk
        // inert. Without this check, disabling the setting would still leave
        // old shells themed because the file's guard would pass.
        if crate::utils::load_config().ui.fish_theme {
            if let Ok(guard) = self.theme_env.lock() {
                if let Some(ref env) = *guard {
                    command.env("FLUX_THEME_FG", &env.fg);
                    command.env("FLUX_THEME_BG", &env.bg);
                    command.env("FLUX_THEME_ACCENT", &env.accent);
                    command.env("FLUX_THEME_MODE", &env.mode);
                    // Palette as a single `,`-separated list of 16 `#rrggbb`
                    // values, indexed 0..15 in xterm order. Keeps the env
                    // small and easy to consume from fish with `string split`.
                    let joined = env.palette.join(";");
                    command.env("FLUX_THEME_PALETTE", joined);
                }
            }
        }

        if let Some(dir) = &working_dir {
            if !dir.is_empty() && std::path::Path::new(dir).is_dir() {
                command.current_dir(dir);
            } else if !dir.is_empty() {
                eprintln!("[terminal] working_dir '{dir}' is not a valid directory, ignoring");
            }
        }

        unsafe {
            command
                .stdin(Stdio::from_raw_fd(libc::dup(slave_fd)))
                .stdout(Stdio::from_raw_fd(libc::dup(slave_fd)))
                .stderr(Stdio::from_raw_fd(libc::dup(slave_fd)))
                .pre_exec(move || {
                    if libc::setsid() < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
        }

        match command.spawn() {
            Ok(mut child) => {
                // Parent closes its copy of slave_fd so EOF propagates on master_fd when the child exits
                unsafe { libc::close(slave_fd) };

                let pid = glib::Pid(child.id() as i32);

                {
                    let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                    state.pty_fd = Some(master_fd);
                    state.shell_pid = Some(child.id() as libc::pid_t);
                }

                let state_clone = self.state.clone();
                let needs_redraw_reader = self.needs_redraw.clone();

                let handle = std::thread::spawn(move || {
                    let mut buf = [0u8; 4096];
                    let mut parser = Parser::new();
                    let mut handler = TerminalHandler {
                        state: state_clone.clone(),
                        needs_redraw: needs_redraw_reader,
                    };
                    loop {
                        let n = unsafe {
                            libc::read(master_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
                        };
                        if n <= 0 {
                            break;
                        }
                        parser.advance(&mut handler, &buf[..n as usize]);
                    }
                });
                self._pty_reader = Some(handle);

                std::thread::spawn(move || {
                    let _ = child.wait();
                });

                callback(Ok(pid));
            }
            Err(e) => {
                unsafe {
                    libc::close(slave_fd);
                    libc::close(master_fd);
                };
                callback(Err(glib::Error::new(
                    glib::FileError::Failed,
                    &e.to_string(),
                )));
            }
        }
    }
}
