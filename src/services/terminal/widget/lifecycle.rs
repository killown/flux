use super::super::state::Cell;
use super::super::Terminal;
use gtk::glib;
use std::sync::Arc;

impl Terminal {
    /// Kills the shell process and cleans up the PTY.
    /// Safe to call multiple times, does nothing if no shell is running.
    pub fn kill_shell(&self) {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return,
        };

        if state.shell_pid.is_none() {
            return;
        }

        if let Some(pid) = state.shell_pid.take() {
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }

        if let Some(fd) = state.pty_master_fd.take() {
            unsafe {
                libc::close(fd);
            }
        }
        state.pty_fd = None;

        // Clear the grid and scrollback so old output doesn't reappear.
        let cols = state.cols;
        let rows = state.rows;
        state.grid = (0..rows).map(|_| vec![Cell::blank(); cols]).collect();
        state.scrollback.clear();
        state.scroll_offset = 0;
        state.cursor_x = 0;
        state.cursor_y = 0;
        state.pending_wrap = false;
    }

    /// Schedules a shell respawn in the given directory.
    ///
    /// Writes the target path into a shared slot and returns immediately,
    /// the main thread is never blocked. A 150 ms debounce timer fires once
    /// navigation settles, only the last directory wins, so rapid folder
    /// traversal never queues multiple respawns.
    pub fn respawn(&self, working_dir: &str) {
        *self.pending_dir.lock().unwrap() = Some(working_dir.to_owned());

        let pending = self.pending_dir.clone();
        let mut term = self.clone();

        glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
            let dir = pending.lock().unwrap().take();
            if let Some(dir) = dir {
                if !term.state.lock().unwrap().is_idle() {
                    return;
                }
                {
                    let mut state = term.state.lock().unwrap();
                    if let Some(fd) = state.pty_master_fd.take() {
                        unsafe { libc::close(fd) };
                    }
                    state.pty_fd = None;
                    state.shell_pid = None;
                    let cols = state.cols;
                    let rows = state.rows;
                    state.grid = (0..rows).map(|_| vec![Cell::blank(); cols]).collect();
                    state.scrollback.clear();
                    state.scroll_offset = 0;
                    state.cursor_x = 0;
                    state.cursor_y = 0;
                    state.pending_wrap = false;
                }
                term.spawn_async(
                    0,
                    Some(&dir),
                    &[],
                    &[],
                    0,
                    || {},
                    -1,
                    None,
                    |result| {
                        if let Err(e) = result {
                            eprintln!("[terminal] respawn failed: {e}");
                        }
                    },
                );
            }
        });
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Only clean up if we are the last reference.
        if Arc::strong_count(&self.state) > 1 {
            return;
        }

        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return,
        };

        if state.cleaned_up {
            return;
        }
        state.cleaned_up = true;

        if let Some(pid) = state.shell_pid.take() {
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }
        if let Some(fd) = state.pty_master_fd.take() {
            unsafe {
                libc::close(fd);
            }
        }
        state.pty_fd = None;
    }
}
