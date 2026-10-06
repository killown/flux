//! Shell process spawning over a PTY.

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
        layout.set_text("W");
        let extents = layout.pixel_extents();
        let char_width = extents.1.width() as f64;
        let char_height = extents.1.height() as f64;

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
        command.env_remove("LD_LIBRARY_PATH");
        command.env_remove("LD_PRELOAD");

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
