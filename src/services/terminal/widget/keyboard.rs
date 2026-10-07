use super::super::state::pty_is_raw;
use super::super::Terminal;
use gtk::gio;
use gtk::prelude::*;
use std::sync::Arc;
use std::sync::Mutex;

/// Writes `data` to the PTY file descriptor without blocking.
#[inline]
fn pty_write(fd: std::os::unix::io::RawFd, data: &[u8]) {
    unsafe {
        libc::write(fd, data.as_ptr() as *const libc::c_void, data.len());
    }
}

impl Terminal {
    /// Installs the keyboard `EventControllerKey` on the drawing area.
    pub(super) fn setup_keyboard(
        drawing_area: &gtk::DrawingArea,
        state: &Arc<Mutex<super::super::state::TerminalState>>,
        im_context: &gtk::IMMulticontext,
    ) {
        let state_for_keys = state.clone();
        let drawing_area_for_keys = drawing_area.clone();
        let im_context_for_keys = im_context.clone();
        let key_controller = gtk::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        key_controller.connect_key_pressed(move |ctrl, keyval, _keycode, modifiers| {
            if !drawing_area_for_keys.has_focus() {
                return glib::Propagation::Proceed;
            }

            let is_ctrl = modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK);
            let is_shift = modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK);
            let is_alt = modifiers.contains(gtk::gdk::ModifierType::ALT_MASK);

            // Ctrl+Shift+C - copy selection to clipboard.
            if is_ctrl && is_shift && (keyval == gtk::gdk::Key::c || keyval == gtk::gdk::Key::C) {
                let state = state_for_keys.lock().unwrap();
                let text = state.get_selected_text();
                if !text.is_empty() {
                    if let Some(window) = drawing_area_for_keys.root() {
                        let display = gtk::prelude::RootExt::display(&window);
                        display.clipboard().set_text(&text);
                    }
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+Shift+V - paste from clipboard with optional bracketed-paste wrapping.
            if is_ctrl && is_shift && (keyval == gtk::gdk::Key::v || keyval == gtk::gdk::Key::V) {
                let state_clone = state_for_keys.clone();
                if let Some(window) = drawing_area_for_keys.root() {
                    let display = gtk::prelude::RootExt::display(&window);
                    display
                        .clipboard()
                        .read_text_async(gio::Cancellable::NONE, move |result| {
                            if let Ok(Some(text)) = result {
                                let state = state_clone.lock().unwrap();
                                if let Some(fd) = state.pty_fd {
                                    if state.bracketed_paste {
                                        pty_write(fd, b"\x1b[200~");
                                    }
                                    pty_write(fd, text.as_bytes());
                                    if state.bracketed_paste {
                                        pty_write(fd, b"\x1b[201~");
                                    }
                                }
                            }
                        });
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+Shift+Up/Down - scroll one line at a time.
            if is_ctrl && is_shift {
                match keyval {
                    gtk::gdk::Key::Up => {
                        state_for_keys.lock().unwrap().scroll_lines(1);
                        drawing_area_for_keys.queue_draw();
                        return glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Down => {
                        state_for_keys.lock().unwrap().scroll_lines(-1);
                        drawing_area_for_keys.queue_draw();
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }
            }

            if let Some(event) = ctrl.current_event() {
                if im_context_for_keys.filter_keypress(&event) {
                    return glib::Propagation::Stop;
                }
            }

            let fd_opt = state_for_keys.lock().unwrap().pty_fd;
            let Some(fd) = fd_opt else {
                return glib::Propagation::Proceed;
            };

            match keyval {
                // ── Scrollback ────────────────────────────────────────────────
                gtk::gdk::Key::Page_Up if is_shift => {
                    state_for_keys.lock().unwrap().scroll_lines(20);
                    drawing_area_for_keys.queue_draw();
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Page_Down if is_shift => {
                    state_for_keys.lock().unwrap().scroll_lines(-20);
                    drawing_area_for_keys.queue_draw();
                    glib::Propagation::Stop
                }

                // ── Basic editing keys ────────────────────────────────────────
                gtk::gdk::Key::BackSpace => {
                    if is_ctrl {
                        pty_write(fd, b"\x17");
                    } else {
                        pty_write(fd, b"\x7f");
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Delete => {
                    if is_ctrl {
                        pty_write(fd, b"\x1b[3;5~");
                    } else {
                        pty_write(fd, b"\x1b[3~");
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                    pty_write(fd, b"\r");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Tab => {
                    if is_shift {
                        pty_write(fd, b"\x1b[Z");
                    } else {
                        pty_write(fd, b"\t");
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Escape => {
                    pty_write(fd, b"\x1b");
                    glib::Propagation::Stop
                }

                // ── Line / word navigation ────────────────────────────────────
                gtk::gdk::Key::Home => {
                    if is_ctrl {
                        let max = state_for_keys.lock().unwrap().scrollback.len();
                        state_for_keys.lock().unwrap().scroll_offset = max;
                        drawing_area_for_keys.queue_draw();
                    } else {
                        pty_write(fd, b"\x1b[H");
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::End => {
                    if is_ctrl {
                        state_for_keys.lock().unwrap().scroll_offset = 0;
                        drawing_area_for_keys.queue_draw();
                    } else {
                        pty_write(fd, b"\x1b[F");
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Insert => {
                    if is_shift {
                        let state_clone = state_for_keys.clone();
                        if let Some(window) = drawing_area_for_keys.root() {
                            let display = gtk::prelude::RootExt::display(&window);
                            display.primary_clipboard().read_text_async(
                                gio::Cancellable::NONE,
                                move |result| {
                                    if let Ok(Some(text)) = result {
                                        let state = state_clone.lock().unwrap();
                                        if let Some(fd) = state.pty_fd {
                                            if state.bracketed_paste {
                                                pty_write(fd, b"\x1b[200~");
                                            }
                                            pty_write(fd, text.as_bytes());
                                            if state.bracketed_paste {
                                                pty_write(fd, b"\x1b[201~");
                                            }
                                        }
                                    }
                                },
                            );
                        }
                    } else {
                        pty_write(fd, b"\x1b[2~");
                    }
                    glib::Propagation::Stop
                }

                // ── Arrow keys ────────────────────────────────────────────────
                gtk::gdk::Key::Up => {
                    let app = state_for_keys.lock().unwrap().application_cursor_keys;
                    let seq: &[u8] = if is_ctrl {
                        b"\x1b[1;5A"
                    } else if app {
                        b"\x1bOA"
                    } else {
                        b"\x1b[A"
                    };
                    pty_write(fd, seq);
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Down => {
                    let app = state_for_keys.lock().unwrap().application_cursor_keys;
                    let seq: &[u8] = if is_ctrl {
                        b"\x1b[1;5B"
                    } else if app {
                        b"\x1bOB"
                    } else {
                        b"\x1b[B"
                    };
                    pty_write(fd, seq);
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Left => {
                    let app = state_for_keys.lock().unwrap().application_cursor_keys;
                    let seq: &[u8] = if is_ctrl {
                        b"\x1b[1;5D"
                    } else if is_alt {
                        b"\x1b[1;3D"
                    } else if app {
                        b"\x1bOD"
                    } else {
                        b"\x1b[D"
                    };
                    pty_write(fd, seq);
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Right => {
                    let app = state_for_keys.lock().unwrap().application_cursor_keys;
                    let seq: &[u8] = if is_ctrl {
                        b"\x1b[1;5C"
                    } else if is_alt {
                        b"\x1b[1;3C"
                    } else if app {
                        b"\x1bOC"
                    } else {
                        b"\x1b[C"
                    };
                    pty_write(fd, seq);
                    glib::Propagation::Stop
                }

                // ── Function keys ─────────────────────────────────────────────
                gtk::gdk::Key::F1 => {
                    pty_write(fd, b"\x1bOP");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::F2 => {
                    pty_write(fd, b"\x1bOQ");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::F3 => {
                    pty_write(fd, b"\x1bOR");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::F4 => {
                    pty_write(fd, b"\x1bOS");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::F5 => {
                    pty_write(fd, b"\x1b[15~");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::F6 => {
                    pty_write(fd, b"\x1b[17~");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::F7 => {
                    pty_write(fd, b"\x1b[18~");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::F8 => {
                    pty_write(fd, b"\x1b[19~");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::F9 => {
                    pty_write(fd, b"\x1b[20~");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::F10 => {
                    pty_write(fd, b"\x1b[21~");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::F11 => {
                    pty_write(fd, b"\x1b[23~");
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::F12 => {
                    pty_write(fd, b"\x1b[24~");
                    glib::Propagation::Stop
                }

                // ── Ctrl + letter ─────────────────────────────────────────────
                _ if is_ctrl && !is_shift => {
                    if let Some(ch) = keyval.to_unicode() {
                        let lower = ch.to_ascii_lowercase();
                        if lower.is_ascii_lowercase() {
                            let ctrl_byte = (lower as u8) - b'a' + 1;
                            if pty_is_raw(fd) {
                                pty_write(fd, &[ctrl_byte]);
                            } else {
                                match ctrl_byte {
                                    0x01 => pty_write(fd, b"\x01"),
                                    0x02 => pty_write(fd, b"\x02"),
                                    0x03 => pty_write(fd, b"\x03"),
                                    0x04 => pty_write(fd, b"\x04"),
                                    0x05 => pty_write(fd, b"\x05"),
                                    0x06 => pty_write(fd, b"\x06"),
                                    0x0b => pty_write(fd, b"\x0b"),
                                    0x0c => pty_write(fd, b"\x0c"),
                                    0x0e => pty_write(fd, b"\x0e"),
                                    0x10 => pty_write(fd, b"\x10"),
                                    0x12 => pty_write(fd, b"\x12"),
                                    0x14 => pty_write(fd, b"\x14"),
                                    0x15 => pty_write(fd, b"\x15"),
                                    0x17 => pty_write(fd, b"\x17"),
                                    0x19 => pty_write(fd, b"\x19"),
                                    0x1a => pty_write(fd, b"\x1a"),
                                    _ => pty_write(fd, &[ctrl_byte]),
                                }
                            }
                            return glib::Propagation::Stop;
                        }
                    }
                    glib::Propagation::Proceed
                }

                // ── Printable / UTF-8 input ───────────────────────────────────
                _ => {
                    if is_alt {
                        if let Some(ch) = keyval.to_unicode() {
                            if !ch.is_control() {
                                let mut seq = [0u8; 5];
                                seq[0] = 0x1b;
                                let mut tmp = [0u8; 4];
                                let n = ch.encode_utf8(&mut tmp).len();
                                seq[1..1 + n].copy_from_slice(&tmp[..n]);
                                pty_write(fd, &seq[..1 + n]);
                                return glib::Propagation::Stop;
                            }
                        }
                        return glib::Propagation::Proceed;
                    }

                    if let Some(ch) = keyval.to_unicode() {
                        if !ch.is_control() {
                            let mut buf = [0u8; 4];
                            let bytes = ch.encode_utf8(&mut buf);
                            pty_write(fd, bytes.as_bytes());
                            return glib::Propagation::Stop;
                        }
                    }

                    glib::Propagation::Proceed
                }
            }
        });
        drawing_area.add_controller(key_controller);
    }
}
