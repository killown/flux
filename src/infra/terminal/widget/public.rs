use super::super::Terminal;
use gtk::prelude::*;

impl Terminal {
    /// Writes raw bytes to the child PTY.
    pub fn feed_child(&self, data: &[u8]) {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.write_pty(data);
    }

    /// Registers a callback invoked on the PTY reader thread whenever fish
    /// reports a working-directory change via OSC 7.
    ///
    /// The callback receives the decoded, absolute path of the new directory.
    /// Register it before the widget is realized so the first prompt isn't
    /// missed. It runs on the PTY reader thread, so it must be `Send`: send a
    /// message through a channel (e.g. `relm4::Sender`) instead of touching GTK
    /// objects directly.
    ///
    /// # Example
    ///
    /// ```ignore
    /// terminal.set_cwd_callback({
    ///     let sender = app_sender.clone();
    ///     move |path| {
    ///         let _ = sender.send(AppMsg::NavigateTo(path));
    ///     }
    /// });
    /// ```
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
}
