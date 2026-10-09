use super::super::draw::draw_terminal;
use super::super::state::TerminalState;
use super::super::Terminal;
use crate::model::TerminalConfig;
use gtk::prelude::*;
use gtk::DrawingArea;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

impl Terminal {
    pub fn new(config: &TerminalConfig) -> Self {
        let drawing_area = DrawingArea::new();
        drawing_area.set_vexpand(true);
        drawing_area.set_hexpand(true);
        drawing_area.set_focusable(true);
        drawing_area.set_can_focus(true);

        let font_desc = pango::FontDescription::from_string(&config.font);
        Self::setup_min_height_on_first_map(&drawing_area, &font_desc);

        let im_context = gtk::IMMulticontext::new();
        im_context.set_use_preedit(false);

        let needs_redraw = Arc::new(AtomicBool::new(false));

        let state = Arc::new(Mutex::new(TerminalState::new(80, 24)));
        state.lock().unwrap().font_desc = font_desc.clone();

        Self::setup_timers_while_mapped(&drawing_area, &needs_redraw, &state);
        Self::setup_im_context(&drawing_area, &state, &im_context);
        Self::setup_draw_func(&drawing_area, &state);
        Self::setup_keyboard(&drawing_area, &state, &im_context);
        Self::setup_mouse(&drawing_area, &state);

        Self {
            drawing_area,
            state,
            config: config.clone(),
            _pty_reader: None,
            needs_redraw,
            pending_dir: Arc::new(Mutex::new(None)),
        }
    }

    /// Measures the font height on first map so startup skips the pango font load.
    fn setup_min_height_on_first_map(
        drawing_area: &gtk::DrawingArea,
        font_desc: &pango::FontDescription,
    ) {
        let font_desc = font_desc.clone();
        let done = Rc::new(Cell::new(false));
        drawing_area.connect_map(move |area| {
            if done.replace(true) {
                return;
            }
            let ctx = pangocairo::FontMap::default().create_context();
            let layout = pango::Layout::new(&ctx);
            layout.set_font_description(Some(&font_desc));
            layout.set_text("M");
            let char_height = layout.pixel_extents().1.height().max(1);
            area.set_size_request(-1, char_height);
        });
    }

    /// Runs the redraw and blink timers only while the widget is mapped.
    fn setup_timers_while_mapped(
        drawing_area: &gtk::DrawingArea,
        needs_redraw: &Arc<AtomicBool>,
        state: &Arc<Mutex<TerminalState>>,
    ) {
        let sources: Rc<RefCell<Vec<glib::SourceId>>> = Rc::new(RefCell::new(Vec::new()));

        {
            let sources = sources.clone();
            let needs_redraw = needs_redraw.clone();
            let state = state.clone();
            drawing_area.connect_map(move |area| {
                let mut sources = sources.borrow_mut();
                if !sources.is_empty() {
                    return;
                }

                // Output may have arrived while hidden.
                area.queue_draw();

                let area_redraw = area.clone();
                let needs_redraw = needs_redraw.clone();
                sources.push(glib::timeout_add_local(
                    std::time::Duration::from_millis(16), // ~60 FPS
                    move || {
                        if needs_redraw.swap(false, Ordering::AcqRel) {
                            area_redraw.queue_draw();
                        }
                        glib::ControlFlow::Continue
                    },
                ));

                let area_blink = area.clone();
                let state = state.clone();
                sources.push(glib::timeout_add_local(
                    std::time::Duration::from_millis(500),
                    move || {
                        {
                            let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                            s.cursor_blink_visible = !s.cursor_blink_visible;
                        }
                        area_blink.queue_allocate();
                        glib::ControlFlow::Continue
                    },
                ));
            });
        }

        drawing_area.connect_unmap(move |_| {
            for id in sources.borrow_mut().drain(..) {
                id.remove();
            }
        });
    }

    fn setup_im_context(
        drawing_area: &gtk::DrawingArea,
        state: &Arc<Mutex<TerminalState>>,
        im_context: &gtk::IMMulticontext,
    ) {
        // Commit handler.
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

        // Realize / unrealize - bind the IM client widget.
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

        // Focus in/out notifications.
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
    }

    fn setup_draw_func(drawing_area: &gtk::DrawingArea, state: &Arc<Mutex<TerminalState>>) {
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

            if new_cols > 0 && new_rows > 0 && (new_cols != state.cols || new_rows != state.rows) {
                state.resize(new_cols, new_rows);
                if let Some(pid) = state.shell_pid {
                    unsafe {
                        libc::kill(pid, libc::SIGWINCH);
                    }
                }
                state.needs_initial_sigwinch = false;
            }

            draw_terminal(area, cr, &state, width, height);
        });
    }
}
