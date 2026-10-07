use super::super::Terminal;
use gtk::prelude::*;

impl Terminal {
    /// Writes raw bytes to the child PTY.
    pub fn feed_child(&self, data: &[u8]) {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.write_pty(data);
    }

    /// Registers a callback invoked whenever the shell's CWD changes.
    pub fn set_cwd_callback<F>(&self, f: F)
    where
        F: Fn(std::path::PathBuf) + Send + 'static,
    {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .on_cwd_change = Some(Box::new(f));
    }

    /// Moves keyboard focus into the terminal's drawing area.
    pub fn grab_focus(&self) {
        self.drawing_area.grab_focus();
    }

    /// Returns `true` if the terminal's drawing area currently holds keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.drawing_area.has_focus()
    }

    /// Sends `SIGWINCH` to the shell process so it re-reads `$LINES`/`$COLUMNS`
    /// from `TIOCGWINSZ`. Call this after the pane has settled at its final size.
    pub fn send_sigwinch(&self) {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pid) = state.shell_pid {
            unsafe {
                libc::kill(pid, libc::SIGWINCH);
            }
        }
    }

    /// Attaches an extra event controller to the drawing area.
    pub fn add_controller(&self, controller: &(impl IsA<gtk::EventController> + Clone)) {
        self.drawing_area.add_controller(controller.clone());
    }

    /// Returns the raw PTY master fd, if any. Test/debug helper.
    #[allow(dead_code)]
    pub fn pty(&self) -> Option<std::os::unix::io::RawFd> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).pty_fd
    }
}
