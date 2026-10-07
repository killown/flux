use super::super::Terminal;
use gtk::gio;
use gtk::prelude::*;

impl Terminal {
    /// Copies the current terminal selection to the CLIPBOARD selection.
    pub fn emit_copy_clipboard(&self) {
        let text = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_selected_text();
        if text.is_empty() {
            return;
        }
        if let Some(display) = gtk::gdk::Display::default() {
            display.clipboard().set_text(&text);
        }
    }

    /// Reads the CLIPBOARD selection and writes it to the PTY, honouring the
    /// application's bracketed-paste mode.
    pub fn emit_paste_clipboard(&self) {
        let state = self.state.clone();
        if let Some(display) = gtk::gdk::Display::default() {
            let clipboard = display.clipboard();
            clipboard.read_text_async(gio::Cancellable::NONE, move |result| {
                if let Ok(Some(text)) = result {
                    let s = state.lock().unwrap();
                    if let Some(fd) = s.pty_fd {
                        let write = |data: &[u8]| unsafe {
                            libc::write(fd, data.as_ptr() as *const libc::c_void, data.len());
                        };
                        if s.bracketed_paste {
                            write(b"\x1b[200~");
                        }
                        write(text.as_bytes());
                        if s.bracketed_paste {
                            write(b"\x1b[201~");
                        }
                    }
                }
            });
        }
    }
}
