use super::super::draw::SCROLLBAR_WIDTH;
use super::super::Terminal;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

impl Terminal {
    /// Installs the four mouse-related controllers on the drawing area.
    pub(super) fn setup_mouse(
        drawing_area: &gtk::DrawingArea,
        state: &Arc<Mutex<super::super::state::TerminalState>>,
    ) {
        let drag_started = Rc::new(RefCell::new(false));

        Self::setup_selection_drag(drawing_area, state, drag_started);
        Self::setup_click_to_focus(drawing_area, state);
        Self::setup_scroll_wheel(drawing_area, state);
        Self::setup_scrollbar_drag(drawing_area, state);
    }

    fn setup_selection_drag(
        drawing_area: &gtk::DrawingArea,
        state: &Arc<Mutex<super::super::state::TerminalState>>,
        drag_started: Rc<RefCell<bool>>,
    ) {
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
        drag_controller.connect_drag_end(move |_gesture, _x, _y| {
            *drag_started.borrow_mut() = false;
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
    }

    fn setup_click_to_focus(
        drawing_area: &gtk::DrawingArea,
        state: &Arc<Mutex<super::super::state::TerminalState>>,
    ) {
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
    }

    fn setup_scroll_wheel(
        drawing_area: &gtk::DrawingArea,
        state: &Arc<Mutex<super::super::state::TerminalState>>,
    ) {
        let state_for_scroll = state.clone();
        let drawing_area_for_scroll = drawing_area.clone();
        let scroll_controller =
            gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll_controller.connect_scroll(move |_controller, _dx, dy| {
            let mut state = state_for_scroll.lock().unwrap();
            if state.scrollback.is_empty() && state.scroll_offset == 0 && dy > 0.0 {
                return glib::Propagation::Proceed;
            }
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
    }

    fn setup_scrollbar_drag(
        drawing_area: &gtk::DrawingArea,
        state: &Arc<Mutex<super::super::state::TerminalState>>,
    ) {
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
    }
}
