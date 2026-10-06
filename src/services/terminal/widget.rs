//! The `Terminal` widget: construction, input handling and lifecycle.

use super::draw::{draw_terminal, SCROLLBAR_WIDTH};
use super::state::{pty_is_raw, Cell, TerminalState};
use super::Terminal;
use crate::model::TerminalConfig;
use gtk::gio;
use gtk::prelude::*;
use gtk::DrawingArea;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

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

        // Send SIGTERM to the shell
        if let Some(pid) = state.shell_pid.take() {
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }

        // Close the PTY master file descriptor
        if let Some(fd) = state.pty_master_fd.take() {
            unsafe {
                libc::close(fd);
            }
        }
        state.pty_fd = None;

        // Clear the grid and scrollback so old output doesn't reappear
        let cols = state.cols;
        let rows = state.rows;
        state.grid = (0..rows).map(|_| vec![Cell::blank(); cols]).collect();
        state.scrollback.clear();
        state.scroll_offset = 0;
        state.cursor_x = 0;
        state.cursor_y = 0;
        state.pending_wrap = false;
    }

    pub fn new(config: &TerminalConfig) -> Self {
        let drawing_area = DrawingArea::new();
        drawing_area.set_vexpand(true);
        drawing_area.set_hexpand(true);
        drawing_area.set_focusable(true);
        drawing_area.set_can_focus(true);

        let font_desc = pango::FontDescription::from_string(&config.font);
        let char_height = {
            let ctx = pangocairo::FontMap::default().create_context();
            let layout = pango::Layout::new(&ctx);
            layout.set_font_description(Some(&font_desc));
            layout.set_text("M");
            layout.pixel_extents().1.height().max(1)
        };
        // Set size request to 1 character line so GTK allows shrinking when resized
        drawing_area.set_size_request(-1, char_height);

        // Input method context
        let im_context = gtk::IMMulticontext::new();
        im_context.set_use_preedit(false);

        // Dirty flag shared between the PTY reader thread (writer) and the GTK
        // main thread (reader).
        let needs_redraw = Arc::new(AtomicBool::new(false));

        let drawing_area_clone = drawing_area.clone();
        let needs_redraw_timer = needs_redraw.clone();
        glib::timeout_add_local(
            std::time::Duration::from_millis(16), // ~60 FPS
            move || {
                if needs_redraw_timer.swap(false, Ordering::AcqRel) {
                    drawing_area_clone.queue_draw();
                }
                glib::ControlFlow::Continue
            },
        );

        let state = Arc::new(Mutex::new(TerminalState::new(80, 24)));
        state.lock().unwrap().font_desc = font_desc.clone();

        // ── Cursor blink timer (~500 ms) ──────────────────────────────────────
        let state_blink = state.clone();
        let drawing_area_blink = drawing_area.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            {
                let mut s = state_blink.lock().unwrap_or_else(|e| e.into_inner());
                s.cursor_blink_visible = !s.cursor_blink_visible;
            }
            drawing_area_blink.queue_allocate();
            glib::ControlFlow::Continue
        });

        {
            let state_for_im = state.clone();
            im_context.connect_commit(move |_, text| {
                let state = state_for_im.lock().unwrap();
                if let Some(fd) = state.pty_fd {
                    unsafe {
                        libc::write(fd, text.as_ptr() as *const libc::c_void, text.len());
                    }
                }
            });
        }

        {
            let im_for_widget = im_context.clone();
            drawing_area.connect_realize(move |w| {
                im_for_widget.set_client_widget(Some(w));
            });
            let im_for_unrealize = im_context.clone();
            drawing_area.connect_unrealize(move |_| {
                im_for_unrealize.set_client_widget(None::<&gtk::Widget>);
            });
        }

        {
            let im_focus = im_context.clone();
            drawing_area.connect_has_focus_notify(move |w| {
                if w.has_focus() {
                    im_focus.focus_in();
                } else {
                    im_focus.focus_out();
                }
            });
        }

        {
            let state = state.clone();
            drawing_area.set_draw_func(move |area, cr, width, height| {
                let mut state = match state.lock() {
                    Ok(s) => s,
                    Err(p) => p.into_inner(), // recover from PTY thread panic
                };
                let layout = area.create_pango_layout(None);
                layout.set_font_description(Some(&state.font_desc));
                layout.set_text("W");
                let extents = layout.pixel_extents();
                let char_width = extents.1.width() as f64;
                let char_height = extents.1.height() as f64;

                let new_cols = (width as f64 / char_width).floor() as usize;
                let new_rows = (height as f64 / char_height).floor() as usize;

                state.char_height = char_height;

                if new_cols > 0
                    && new_rows > 0
                    && (new_cols != state.cols || new_rows != state.rows)
                {
                    state.resize(new_cols, new_rows);
                    // Always send SIGWINCH after resize so fish immediately
                    // redraws at the new dimensions. resize() already issues
                    // TIOCSWINSZ, the explicit SIGWINCH ensures fish re-queries
                    // $LINES/$COLUMNS even when it missed the kernel signal.
                    if let Some(pid) = state.shell_pid {
                        unsafe {
                            libc::kill(pid, libc::SIGWINCH);
                        }
                    }
                    // Clear the deferred-winch flag if it was pending.
                    state.needs_initial_sigwinch = false;
                }

                draw_terminal(area, cr, &state, width, height);
            });
        }

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

            /// Writes `data` to the PTY file descriptor without blocking.
            #[inline]
            fn pty_write(fd: std::os::unix::io::RawFd, data: &[u8]) {
                unsafe {
                    libc::write(fd, data.as_ptr() as *const libc::c_void, data.len());
                }
            }

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
                    // Ctrl+Backspace - delete word to the left (^W in readline/fish).
                    if is_ctrl {
                        pty_write(fd, b"\x17");
                    } else {
                        pty_write(fd, b"\x7f");
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Delete => {
                    // Ctrl+Delete - delete word to the right (\e[3,5~).
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
                    // Shift+Tab - reverse tab / menu-back in completions (\e[Z).
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
                    // Ctrl+Home - scroll to top of scrollback.
                    if is_ctrl {
                        let max = state_for_keys.lock().unwrap().scrollback.len();
                        state_for_keys.lock().unwrap().scroll_offset = max;
                        drawing_area_for_keys.queue_draw();
                    } else {
                        // Move cursor to beginning of line (^A / \e[H).
                        pty_write(fd, b"\x1b[H");
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::End => {
                    // Ctrl+End - scroll back to the live view.
                    if is_ctrl {
                        state_for_keys.lock().unwrap().scroll_offset = 0;
                        drawing_area_for_keys.queue_draw();
                    } else {
                        pty_write(fd, b"\x1b[F");
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Insert => {
                    // Shift+Insert - paste from primary selection.
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
                    // Ctrl+Up - jump word upward in history (\e[1,5A).
                    // DECCKM: application mode sends \eOA instead of \e[A.
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
                    // Ctrl+Left - move one word left (\e[1,5D).
                    // Alt+Left - same, alternate encoding some shells prefer (\e[1,3D).
                    // DECCKM: application mode sends \eOD instead of \e[D.
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

                // ── Ctrl + letter: readline bindings (cooked) or raw pass-through ──
                //
                // When the PTY is in raw/cbreak mode (ICANON=0), e.g. nvim, nano,
                // less, every Ctrl+key must reach the application as its ASCII
                // control byte without any emulator-level interpretation. In cooked
                // mode (fish prompt) we keep the explicit readline bindings so that
                // Ctrl+Z suspends, Ctrl+C interrupts, etc.
                _ if is_ctrl && !is_shift => {
                    if let Some(ch) = keyval.to_unicode() {
                        let lower = ch.to_ascii_lowercase();
                        if lower.is_ascii_lowercase() {
                            let ctrl_byte = (lower as u8) - b'a' + 1;
                            if pty_is_raw(fd) {
                                pty_write(fd, &[ctrl_byte]);
                            } else {
                                match ctrl_byte {
                                    0x01 => pty_write(fd, b"\x01"), // ^A beginning of line
                                    0x02 => pty_write(fd, b"\x02"), // ^B move char left
                                    0x03 => pty_write(fd, b"\x03"), // ^C SIGINT
                                    0x04 => pty_write(fd, b"\x04"), // ^D EOF
                                    0x05 => pty_write(fd, b"\x05"), // ^E end of line
                                    0x06 => pty_write(fd, b"\x06"), // ^F move char right
                                    0x0b => pty_write(fd, b"\x0b"), // ^K kill to EOL
                                    0x0c => pty_write(fd, b"\x0c"), // ^L clear screen
                                    0x0e => pty_write(fd, b"\x0e"), // ^N next history
                                    0x10 => pty_write(fd, b"\x10"), // ^P prev history
                                    0x12 => pty_write(fd, b"\x12"), // ^R reverse search
                                    0x14 => pty_write(fd, b"\x14"), // ^T transpose
                                    0x15 => pty_write(fd, b"\x15"), // ^U kill to BOL
                                    0x17 => pty_write(fd, b"\x17"), // ^W delete word left
                                    0x19 => pty_write(fd, b"\x19"), // ^Y yank
                                    0x1a => pty_write(fd, b"\x1a"), // ^Z SIGTSTP
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
                    // Alt+key - prefix with ESC (\e + byte), used by readline/fish
                    // for word-navigation and meta-bindings (Alt+f, Alt+b, etc.).
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

        // Mouse selection support - using Rc instead of Arc (not Send/Sync)
        use std::cell::RefCell;
        use std::rc::Rc;

        let drag_started = Rc::new(RefCell::new(false));

        let state_for_drag = state.clone();
        let drawing_area_for_drag = drawing_area.clone();
        let drag_controller = gtk::GestureDrag::new();

        let drag_started_clone = drag_started.clone();
        drag_controller.connect_drag_begin(move |gesture, _x, _y| {
            let mut state = state_for_drag.lock().unwrap();
            let (x, y) = gesture.start_point().unwrap_or((0.0, 0.0));

            let layout = drawing_area_for_drag.create_pango_layout(None);
            layout.set_font_description(Some(&state.font_desc));
            layout.set_text("W");
            let extents = layout.pixel_extents();
            let char_width = extents.1.width() as f64;
            let char_height = extents.1.height() as f64;

            let col = (x / char_width) as usize;
            let total_rows = state.scrollback.len() + state.grid.len();
            let row = (y / char_height) as usize;

            let abs_row = if state.scroll_offset > 0 && row < state.scrollback.len() {
                state.scrollback.len().saturating_sub(state.scroll_offset) + row
            } else {
                state.scrollback.len() + row
            };

            let abs_row = abs_row.min(total_rows - 1);
            let col = col.min(state.cols - 1);

            state.selection_start = Some((abs_row, col));
            state.selection_end = Some((abs_row, col));
            state.selection_active = true;
            *drag_started_clone.borrow_mut() = true;
            drawing_area_for_drag.queue_draw();
        });

        let state_for_drag_update = state.clone();
        let drawing_area_for_drag_update = drawing_area.clone();
        let drag_started_clone2 = drag_started.clone();
        drag_controller.connect_drag_update(move |gesture, _x, _y| {
            if !*drag_started_clone2.borrow() {
                return;
            }
            let mut state = state_for_drag_update.lock().unwrap();
            let (x, y) = gesture.point(None).unwrap_or((0.0, 0.0));

            let layout = drawing_area_for_drag_update.create_pango_layout(None);
            layout.set_font_description(Some(&state.font_desc));
            layout.set_text("W");
            let extents = layout.pixel_extents();
            let char_width = extents.1.width() as f64;
            let char_height = extents.1.height() as f64;

            let col = (x / char_width) as usize;
            let total_rows = state.scrollback.len() + state.grid.len();
            let row = (y / char_height) as usize;

            let abs_row = if state.scroll_offset > 0 && row < state.scrollback.len() {
                state.scrollback.len().saturating_sub(state.scroll_offset) + row
            } else {
                state.scrollback.len() + row
            };

            let abs_row = abs_row.min(total_rows - 1);
            let col = col.min(state.cols - 1);

            state.selection_end = Some((abs_row, col));
            drawing_area_for_drag_update.queue_draw();
        });

        let state_for_drag_end = state.clone();
        let drawing_area_for_drag_end = drawing_area.clone();
        let drag_started_clone3 = drag_started;
        drag_controller.connect_drag_end(move |_gesture, _x, _y| {
            *drag_started_clone3.borrow_mut() = false;
            let state = state_for_drag_end.lock().unwrap();
            if state.selection_active {
                let text = state.get_selected_text();
                if !text.is_empty() {
                    if let Some(window) = drawing_area_for_drag_end.root() {
                        let display = gtk::prelude::RootExt::display(&window);
                        let primary_clipboard = display.primary_clipboard();
                        primary_clipboard.set_text(&text);
                    }
                }
            }
            drawing_area_for_drag_end.queue_draw();
        });
        drawing_area.add_controller(drag_controller);

        let state_for_scroll = state.clone();
        let drawing_area_for_scroll = drawing_area.clone();
        let scroll_controller =
            gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll_controller.connect_scroll(move |_controller, _dx, dy| {
            let mut state = state_for_scroll.lock().unwrap();
            if state.scrollback.is_empty() && state.scroll_offset == 0 && dy > 0.0 {
                return glib::Propagation::Proceed;
            }
            // dy > 0 = wheel down = towards newer content (decrease offset).
            // Use signum so even a sub-1.0 touchpad nudge registers as 1 line.
            let lines = if dy.abs() < 1.0 {
                -(dy.signum() as i32)
            } else {
                -(dy.round() as i32)
            };
            state.scroll_lines(lines);
            drawing_area_for_scroll.queue_draw();
            glib::Propagation::Stop
        });
        drawing_area.add_controller(scroll_controller);

        let state_clone = state.clone();
        let drawing_area_focus = drawing_area.clone();
        let click_controller = gtk::GestureClick::new();
        click_controller.set_button(1);
        click_controller.connect_pressed(move |_, _, _, _| {
            let mut state = state_clone.lock().unwrap();
            if !state.selection_active {
                state.selection_start = None;
                state.selection_end = None;
            }
            drawing_area_focus.queue_draw();
            drawing_area_focus.grab_focus();
        });
        drawing_area.add_controller(click_controller);

        // Scrollbar drag: a secondary GestureDrag that only activates when the
        // press lands inside the right SCROLLBAR_WIDTH * 2 hit zone. Translating
        // the Y position of the drag point into a scroll_offset mirrors the
        // inverse of the thumb_y formula in draw_scrollbar.
        let state_for_sb = state.clone();
        let drawing_area_for_sb = drawing_area.clone();
        let sb_drag = gtk::GestureDrag::new();
        sb_drag.set_button(1);
        sb_drag.set_exclusive(true);

        let state_sb_begin = state_for_sb.clone();
        let da_sb_begin = drawing_area_for_sb.clone();
        sb_drag.connect_drag_begin(move |gesture, x, _y| {
            let widget_width = da_sb_begin.width() as f64;
            if x < widget_width - SCROLLBAR_WIDTH * 2.0 {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            let sb = state_sb_begin.lock().unwrap();
            if sb.scrollback.is_empty() {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
        });

        let state_sb_update = state_for_sb.clone();
        let da_sb_update = drawing_area_for_sb.clone();
        sb_drag.connect_drag_update(move |gesture, _dx, _dy| {
            let (_, y) = match gesture.point(None) {
                Some(p) => p,
                None => return,
            };
            let h = da_sb_update.height() as f64;
            if h <= 0.0 {
                return;
            }
            let mut state = state_sb_update.lock().unwrap();
            if state.scrollback.is_empty() {
                return;
            }
            let max_offset = state.scrollback.len();
            let visible_rows = if state.char_height > 0.0 {
                (h / state.char_height).floor() as usize
            } else {
                state.rows
            };
            let total_rows = max_offset + visible_rows;
            let thumb_ratio = (visible_rows as f64 / total_rows as f64).min(1.0);
            let thumb_h = (h * thumb_ratio).max(20.0);
            // Invert draw_scrollbar's thumb_y: thumb_y = h - thumb_h - (h - thumb_h) * frac
            let thumb_y = (y - thumb_h / 2.0).clamp(0.0, h - thumb_h);
            let track_h = h - thumb_h;
            let scroll_frac = if track_h > 0.0 {
                ((h - thumb_h - thumb_y) / track_h).clamp(0.0, 1.0)
            } else {
                0.0
            };
            state.scroll_offset = (scroll_frac * max_offset as f64).round() as usize;
            da_sb_update.queue_draw();
        });

        drawing_area.add_controller(sb_drag);

        Self {
            drawing_area,
            state,
            config: config.clone(),
            _pty_reader: None,
            needs_redraw,
            pending_dir: Arc::new(Mutex::new(None)),
        }
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

        // 150 ms debounce: if another respawn arrives before the timer fires,
        // it overwrites pending_dir and this closure becomes a no-op.
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

    pub fn feed_child(&self, data: &[u8]) {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.write_pty(data);
    }

    pub fn set_cwd_callback<F>(&self, f: F)
    where
        F: Fn(std::path::PathBuf) + Send + 'static,
    {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .on_cwd_change = Some(Box::new(f));
    }

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

    pub fn add_controller(&self, controller: &(impl IsA<gtk::EventController> + Clone)) {
        self.drawing_area.add_controller(controller.clone());
    }

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

    #[allow(dead_code)]
    pub fn pty(&self) -> Option<std::os::unix::io::RawFd> {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).pty_fd
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Only clean up if we are the last reference
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
