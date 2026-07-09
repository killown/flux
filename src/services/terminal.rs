//NOTE: add scrollbar

use gtk::cairo::Context;
use gtk::prelude::*;
use gtk::DrawingArea;
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use vte::{Parser, Perform};

#[derive(Clone, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<gtk::gdk::RGBA>,
    pub bg: Option<gtk::gdk::RGBA>,
    pub bold: bool,
}

#[derive(Debug)]
pub struct TerminalState {
    pub grid: Vec<Vec<Cell>>,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub cols: usize,
    pub rows: usize,
    pub fg_color: gtk::gdk::RGBA,
    pub bg_color: gtk::gdk::RGBA,
    pub font_desc: pango::FontDescription,
    pub pty_fd: Option<std::os::unix::io::RawFd>,
    pub saved_cursor_x: usize,
    pub saved_cursor_y: usize,
    pub current_fg: gtk::gdk::RGBA,
    pub current_bg: gtk::gdk::RGBA,
    pub bold: bool,
    pub scrollback: Vec<Vec<Cell>>,
    pub scrollback_limit: usize,
    pub scroll_offset: usize,
    pub selection_start: Option<(usize, usize)>,
    pub selection_end: Option<(usize, usize)>,
    pub selection_active: bool,
}

impl TerminalState {
    pub fn new(cols: usize, rows: usize) -> Self {
        let mut grid = Vec::with_capacity(rows);
        for _ in 0..rows {
            let mut row = Vec::with_capacity(cols);
            for _ in 0..cols {
                row.push(Cell {
                    ch: ' ',
                    fg: None,
                    bg: None,
                    bold: false,
                });
            }
            grid.push(row);
        }
        let fg = gtk::gdk::RGBA::new(0.9, 0.9, 0.9, 1.0);
        let bg = gtk::gdk::RGBA::new(0.1, 0.1, 0.1, 1.0);
        Self {
            grid,
            cursor_x: 0,
            cursor_y: 0,
            cols,
            rows,
            fg_color: fg.clone(),
            bg_color: bg.clone(),
            font_desc: pango::FontDescription::from_string("JetBrains Mono 13"),
            pty_fd: None,
            saved_cursor_x: 0,
            saved_cursor_y: 0,
            current_fg: fg,
            current_bg: bg,
            bold: false,
            scrollback: Vec::new(),
            scrollback_limit: 10000,
            scroll_offset: 0,
            selection_start: None,
            selection_end: None,
            selection_active: false,
        }
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        if cols == self.cols && rows == self.rows {
            return;
        }

        for row in self.scrollback.iter_mut() {
            if row.len() != cols {
                let mut new_row = Vec::with_capacity(cols);
                for x in 0..cols {
                    let cell = if x < row.len() {
                        row[x].clone()
                    } else {
                        Cell {
                            ch: ' ',
                            fg: None,
                            bg: None,
                            bold: false,
                        }
                    };
                    new_row.push(cell);
                }
                *row = new_row;
            }
        }

        let mut new_grid = Vec::with_capacity(rows);
        for y in 0..rows {
            let mut row = Vec::with_capacity(cols);
            for x in 0..cols {
                let cell = if y < self.rows && x < self.cols {
                    self.grid[y][x].clone()
                } else {
                    Cell {
                        ch: ' ',
                        fg: None,
                        bg: None,
                        bold: false,
                    }
                };
                row.push(cell);
            }
            new_grid.push(row);
        }

        self.grid = new_grid;
        self.cols = cols;
        self.rows = rows;
        if self.cursor_x >= cols {
            self.cursor_x = cols - 1;
        }
        if self.cursor_y >= rows {
            self.cursor_y = rows - 1;
        }
        self.selection_start = None;
        self.selection_end = None;
        self.selection_active = false;
    }

    pub fn scroll_up(&mut self) {
        if self.grid.is_empty() {
            return;
        }
        let top_row = self.grid.remove(0);
        self.scrollback.push(top_row);
        if self.scrollback.len() > self.scrollback_limit {
            self.scrollback.remove(0);
        }

        let empty_row = vec![
            Cell {
                ch: ' ',
                fg: None,
                bg: None,
                bold: false
            };
            self.cols
        ];
        self.grid.push(empty_row);

        if self.scroll_offset > 0 {
            self.scroll_offset = self.scrollback.len().min(self.scroll_offset + 1);
        }
    }

    pub fn write_pty(&self, data: &[u8]) -> bool {
        if let Some(fd) = self.pty_fd {
            unsafe {
                let n = libc::write(fd, data.as_ptr() as *const libc::c_void, data.len());
                n == data.len() as isize
            }
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        for row in self.grid.iter_mut() {
            for cell in row.iter_mut() {
                cell.ch = ' ';
                cell.fg = None;
                cell.bg = None;
                cell.bold = false;
            }
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.current_fg = self.fg_color.clone();
        self.current_bg = self.bg_color.clone();
        self.bold = false;
        self.scrollback.clear();
        self.scroll_offset = 0;
        self.selection_start = None;
        self.selection_end = None;
        self.selection_active = false;
    }

    pub fn scroll_lines(&mut self, lines: i32) {
        let max_offset = self.scrollback.len();
        if lines > 0 {
            self.scroll_offset = (self.scroll_offset + lines as usize).min(max_offset);
        } else {
            let lines = (-lines) as usize;
            self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        }
    }

    pub fn get_selected_text(&self) -> String {
        let (start, end) = match (self.selection_start, self.selection_end) {
            (Some(s), Some(e)) => (s, e),
            _ => return String::new(),
        };

        let (row1, col1) = if start < end { start } else { end };
        let (row2, col2) = if start < end { end } else { start };

        let mut text = String::new();
        for row in row1..=row2 {
            let row_cells = if row < self.scrollback.len() {
                self.scrollback.get(row)
            } else {
                let grid_row = row - self.scrollback.len();
                self.grid.get(grid_row)
            };

            if let Some(row_cells) = row_cells {
                let start_col = if row == row1 { col1 } else { 0 };
                let end_col = if row == row2 { col2 } else { self.cols - 1 };
                for col in start_col..=end_col {
                    if let Some(cell) = row_cells.get(col) {
                        text.push(cell.ch);
                    }
                }
                if row < row2 {
                    text.push('\n');
                }
            }
        }
        text
    }
}

pub struct TerminalHandler {
    pub state: Arc<Mutex<TerminalState>>,
    pub draw_sender: tokio::sync::mpsc::UnboundedSender<()>,
}

impl Perform for TerminalHandler {
    fn print(&mut self, c: char) {
        let mut state = self.state.lock().unwrap();
        let y = state.cursor_y;
        let x = state.cursor_x;
        if x < state.cols && y < state.rows {
            state.grid[y][x].ch = c;
            state.grid[y][x].fg = Some(state.current_fg.clone());
            state.grid[y][x].bg = Some(state.current_bg.clone());
            state.grid[y][x].bold = state.bold;
        }
        state.cursor_x += 1;
        if state.cursor_x >= state.cols {
            state.cursor_x = 0;
            state.cursor_y += 1;
            if state.cursor_y >= state.rows {
                state.scroll_up();
                state.cursor_y = state.rows - 1;
            }
        }
        state.selection_active = false;
        state.selection_start = None;
        state.selection_end = None;
        let _ = self.draw_sender.send(());
    }

    fn execute(&mut self, byte: u8) {
        let mut state = self.state.lock().unwrap();
        match byte {
            b'\r' => state.cursor_x = 0,
            b'\n' => {
                state.cursor_y += 1;
                if state.cursor_y >= state.rows {
                    state.scroll_up();
                    state.cursor_y = state.rows - 1;
                }
            }
            b'\t' => {
                state.cursor_x = ((state.cursor_x / 8) + 1) * 8;
                if state.cursor_x >= state.cols {
                    state.cursor_x = state.cols - 1;
                }
            }
            b'\x08' => {
                if state.cursor_x > 0 {
                    state.cursor_x -= 1;
                }
            }
            b'\x0c' => {
                state.clear();
            }
            b'\x03' => {
                if let Some(fd) = state.pty_fd {
                    let _ = unsafe { libc::write(fd, b"\x03".as_ptr() as *const libc::c_void, 1) };
                }
            }
            _ => {}
        }
        let _ = self.draw_sender.send(());
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        command: char,
    ) {
        let mut state = self.state.lock().unwrap();

        let mut p = Vec::new();
        for param in params.iter() {
            if let Some(&val) = param.get(0) {
                p.push(val as i64);
            }
        }

        if command == 'c' {
            let is_secondary = !intermediates.is_empty() && intermediates[0] == b'>';

            let response: &[u8] = if is_secondary {
                b"\x1b[>0;0;0c"
            } else {
                b"\x1b[?1;0c"
            };

            if let Some(fd) = state.pty_fd {
                let _ = unsafe {
                    libc::write(fd, response.as_ptr() as *const libc::c_void, response.len())
                };
            }
            return;
        }

        match command {
            'A' => {
                let n = p.get(0).map(|&v| v as usize).unwrap_or(1);
                state.cursor_y = state.cursor_y.saturating_sub(n);
            }
            'B' => {
                let n = p.get(0).map(|&v| v as usize).unwrap_or(1);
                state.cursor_y = (state.cursor_y + n).min(state.rows - 1);
            }
            'C' => {
                let n = p.get(0).map(|&v| v as usize).unwrap_or(1);
                state.cursor_x = (state.cursor_x + n).min(state.cols - 1);
            }
            'D' => {
                let n = p.get(0).map(|&v| v as usize).unwrap_or(1);
                state.cursor_x = state.cursor_x.saturating_sub(n);
            }
            'H' | 'f' => {
                let row = p.get(0).map(|&v| v as usize).unwrap_or(1);
                let col = p.get(1).map(|&v| v as usize).unwrap_or(1);
                state.cursor_y = (row - 1).min(state.rows - 1);
                state.cursor_x = (col - 1).min(state.cols - 1);
            }
            'J' => match p.get(0).map(|&v| v).unwrap_or(0) {
                0 => {
                    for y in state.cursor_y..state.rows {
                        for x in 0..state.cols {
                            if y == state.cursor_y && x < state.cursor_x {
                                continue;
                            }
                            state.grid[y][x].ch = ' ';
                            state.grid[y][x].fg = None;
                            state.grid[y][x].bg = None;
                            state.grid[y][x].bold = false;
                        }
                    }
                }
                1 => {
                    for y in 0..=state.cursor_y {
                        for x in 0..state.cols {
                            if y == state.cursor_y && x > state.cursor_x {
                                continue;
                            }
                            state.grid[y][x].ch = ' ';
                            state.grid[y][x].fg = None;
                            state.grid[y][x].bg = None;
                            state.grid[y][x].bold = false;
                        }
                    }
                }
                2 => {
                    state.clear();
                }
                _ => {}
            },
            'K' => {
                let row = state.cursor_y;
                match p.get(0).map(|&v| v).unwrap_or(0) {
                    0 => {
                        for x in state.cursor_x..state.cols {
                            state.grid[row][x].ch = ' ';
                            state.grid[row][x].fg = None;
                            state.grid[row][x].bg = None;
                            state.grid[row][x].bold = false;
                        }
                    }
                    1 => {
                        for x in 0..=state.cursor_x {
                            state.grid[row][x].ch = ' ';
                            state.grid[row][x].fg = None;
                            state.grid[row][x].bg = None;
                            state.grid[row][x].bold = false;
                        }
                    }
                    2 => {
                        for x in 0..state.cols {
                            state.grid[row][x].ch = ' ';
                            state.grid[row][x].fg = None;
                            state.grid[row][x].bg = None;
                            state.grid[row][x].bold = false;
                        }
                    }
                    _ => {}
                }
            }
            'm' => {
                if p.is_empty() || p[0] == 0 {
                    state.current_fg = state.fg_color.clone();
                    state.current_bg = state.bg_color.clone();
                    state.bold = false;
                } else {
                    let mut i = 0;
                    while i < p.len() {
                        match p[i] {
                            0 => {
                                state.current_fg = state.fg_color.clone();
                                state.current_bg = state.bg_color.clone();
                                state.bold = false;
                            }
                            1 => state.bold = true,
                            30..=37 => {
                                let colors = [
                                    (0.0, 0.0, 0.0),
                                    (0.8, 0.0, 0.0),
                                    (0.0, 0.8, 0.0),
                                    (0.8, 0.8, 0.0),
                                    (0.0, 0.0, 0.8),
                                    (0.8, 0.0, 0.8),
                                    (0.0, 0.8, 0.8),
                                    (0.8, 0.8, 0.8),
                                ];
                                let idx = (p[i] - 30) as usize;
                                if idx < colors.len() {
                                    let (r, g, b) = colors[idx];
                                    state.current_fg = gtk::gdk::RGBA::new(r, g, b, 1.0);
                                }
                            }
                            40..=47 => {
                                let colors = [
                                    (0.0, 0.0, 0.0),
                                    (0.8, 0.0, 0.0),
                                    (0.0, 0.8, 0.0),
                                    (0.8, 0.8, 0.0),
                                    (0.0, 0.0, 0.8),
                                    (0.8, 0.0, 0.8),
                                    (0.0, 0.8, 0.8),
                                    (0.8, 0.8, 0.8),
                                ];
                                let idx = (p[i] - 40) as usize;
                                if idx < colors.len() {
                                    let (r, g, b) = colors[idx];
                                    state.current_bg = gtk::gdk::RGBA::new(r, g, b, 1.0);
                                }
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                }
            }
            's' => {
                state.saved_cursor_x = state.cursor_x;
                state.saved_cursor_y = state.cursor_y;
            }
            'u' => {
                state.cursor_x = state.saved_cursor_x;
                state.cursor_y = state.saved_cursor_y;
            }
            _ => {}
        }
        let _ = self.draw_sender.send(());
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]], _command: bool) {
        // Ignore OSC sequences for now
    }
}

pub struct Terminal {
    pub drawing_area: DrawingArea,
    pub state: Arc<Mutex<TerminalState>>,
    _pty_reader: Option<std::thread::JoinHandle<()>>,
    draw_sender: tokio::sync::mpsc::UnboundedSender<()>,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("drawing_area", &self.drawing_area)
            .finish()
    }
}

impl Clone for Terminal {
    fn clone(&self) -> Self {
        Self {
            drawing_area: self.drawing_area.clone(),
            state: self.state.clone(),
            _pty_reader: None,
            draw_sender: self.draw_sender.clone(),
        }
    }
}

impl Terminal {
    pub fn new() -> Self {
        let drawing_area = DrawingArea::new();
        drawing_area.set_vexpand(true);
        drawing_area.set_hexpand(true);
        drawing_area.set_focusable(true);
        drawing_area.set_can_focus(true);
        drawing_area.set_height_request(200);

        let (draw_sender, mut draw_receiver) = tokio::sync::mpsc::unbounded_channel::<()>();

        let drawing_area_clone = drawing_area.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Some(()) = draw_receiver.recv().await {
                drawing_area_clone.queue_draw();
            }
        });

        let state = Arc::new(Mutex::new(TerminalState::new(80, 24)));

        {
            let state = state.clone();
            drawing_area.set_draw_func(move |area, cr, width, height| {
                let mut state = state.lock().unwrap();
                let layout = area.create_pango_layout(None);
                layout.set_font_description(Some(&state.font_desc));
                layout.set_text("W");
                let extents = layout.pixel_extents();
                let char_width = extents.1.width() as f64;
                let char_height = extents.1.height() as f64;

                let new_cols = (width as f64 / char_width).floor() as usize;
                let new_rows = (height as f64 / char_height).floor() as usize;

                if new_cols > 0 && new_rows > 0 {
                    state.resize(new_cols, new_rows);
                }

                draw_terminal(area, cr, &state, width, height);
            });
        }

        let state_for_keys = state.clone();
        let drawing_area_for_keys = drawing_area.clone();
        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed(move |_ctrl, keyval, _keycode, modifiers| {
            if !drawing_area_for_keys.has_focus() {
                return glib::Propagation::Proceed;
            }

            let is_ctrl = modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK);
            let is_shift = modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK);

            // Ctrl+Shift+C to copy selection
            if is_ctrl && is_shift && (keyval == gtk::gdk::Key::c || keyval == gtk::gdk::Key::C) {
                let state = state_for_keys.lock().unwrap();
                let text = state.get_selected_text();
                if !text.is_empty() {
                    if let Some(window) = drawing_area_for_keys.root() {
                        let display = gtk::prelude::RootExt::display(&window);
                        let clipboard = display.clipboard();
                        clipboard.set_text(&text);
                    }
                }
                return glib::Propagation::Stop;
            }

            match keyval {
                gtk::gdk::Key::Page_Up if is_shift => {
                    let mut state = state_for_keys.lock().unwrap();
                    state.scroll_lines(20);
                    drawing_area_for_keys.queue_draw();
                    return glib::Propagation::Stop;
                }
                gtk::gdk::Key::Page_Down if is_shift => {
                    let mut state = state_for_keys.lock().unwrap();
                    state.scroll_lines(-20);
                    drawing_area_for_keys.queue_draw();
                    return glib::Propagation::Stop;
                }
                gtk::gdk::Key::BackSpace => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ =
                            unsafe { libc::write(fd, b"\x7f".as_ptr() as *const libc::c_void, 1) };
                    }
                    return glib::Propagation::Stop;
                }
                gtk::gdk::Key::Return => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ =
                            unsafe { libc::write(fd, b"\r".as_ptr() as *const libc::c_void, 1) };
                    }
                    return glib::Propagation::Stop;
                }
                gtk::gdk::Key::Tab => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ =
                            unsafe { libc::write(fd, b"\t".as_ptr() as *const libc::c_void, 1) };
                    }
                    return glib::Propagation::Stop;
                }
                gtk::gdk::Key::c | gtk::gdk::Key::C if is_ctrl => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ =
                            unsafe { libc::write(fd, b"\x03".as_ptr() as *const libc::c_void, 1) };
                    }
                    return glib::Propagation::Stop;
                }
                gtk::gdk::Key::d | gtk::gdk::Key::D if is_ctrl => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ =
                            unsafe { libc::write(fd, b"\x04".as_ptr() as *const libc::c_void, 1) };
                    }
                    return glib::Propagation::Stop;
                }
                gtk::gdk::Key::Up => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ = unsafe {
                            libc::write(fd, b"\x1b[A".as_ptr() as *const libc::c_void, 3)
                        };
                    }
                    return glib::Propagation::Stop;
                }
                gtk::gdk::Key::Down => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ = unsafe {
                            libc::write(fd, b"\x1b[B".as_ptr() as *const libc::c_void, 3)
                        };
                    }
                    return glib::Propagation::Stop;
                }
                gtk::gdk::Key::Left => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ = unsafe {
                            libc::write(fd, b"\x1b[D".as_ptr() as *const libc::c_void, 3)
                        };
                    }
                    return glib::Propagation::Stop;
                }
                gtk::gdk::Key::Right => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ = unsafe {
                            libc::write(fd, b"\x1b[C".as_ptr() as *const libc::c_void, 3)
                        };
                    }
                    return glib::Propagation::Stop;
                }
                _ => {
                    if let Some(ch) = keyval.to_unicode() {
                        if ch.is_ascii_graphic() || ch == ' ' {
                            let mut buf = [0u8; 4];
                            let bytes = ch.encode_utf8(&mut buf);
                            if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                                let _ = unsafe {
                                    libc::write(
                                        fd,
                                        bytes.as_ptr() as *const libc::c_void,
                                        bytes.len(),
                                    )
                                };
                            }
                            return glib::Propagation::Stop;
                        }
                    }
                }
            }
            glib::Propagation::Proceed
        });
        drawing_area.add_controller(key_controller);

        // Mouse selection support
        let drag_started = Arc::new(std::cell::Cell::new(false));

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
            drag_started_clone.set(true);
            drawing_area_for_drag.queue_draw();
        });

        let state_for_drag_update = state.clone();
        let drawing_area_for_drag_update = drawing_area.clone();
        let drag_started_clone2 = drag_started.clone();
        drag_controller.connect_drag_update(move |gesture, _x, _y| {
            if !drag_started_clone2.get() {
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
            drag_started_clone3.set(false);
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
            if state.scrollback.is_empty() && state.scroll_offset == 0 {
                return glib::Propagation::Proceed;
            }
            let lines = (dy / 3.0) as i32;
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
            state.scroll_offset = 0;
            drawing_area_focus.queue_draw();
            drawing_area_focus.grab_focus();
        });
        drawing_area.add_controller(click_controller);

        let mut term = Self {
            drawing_area,
            state,
            _pty_reader: None,
            draw_sender,
        };

        term.spawn_async(
            0,
            None,
            &[],
            &[],
            0,
            || {},
            -1,
            None,
            |result| {
                if let Err(e) = result {
                    eprintln!("Failed to spawn terminal shell: {}", e);
                }
            },
        );

        term
    }

    pub fn feed_child(&self, data: &[u8]) {
        let state = self.state.lock().unwrap();
        state.write_pty(data);
    }

    pub fn set_color_foreground(&self, color: &gtk::gdk::RGBA) {
        let mut state = self.state.lock().unwrap();
        state.fg_color = color.clone();
        state.current_fg = color.clone();
        self.drawing_area.queue_draw();
    }

    pub fn set_color_background(&self, color: &gtk::gdk::RGBA) {
        let mut state = self.state.lock().unwrap();
        state.bg_color = color.clone();
        state.current_bg = color.clone();
        self.drawing_area.queue_draw();
    }

    pub fn set_font(&self, font_desc: Option<&pango::FontDescription>) {
        if let Some(fd) = font_desc {
            let mut state = self.state.lock().unwrap();
            state.font_desc = fd.clone();
            self.drawing_area.queue_draw();
        }
    }

    pub fn grab_focus(&self) {
        self.drawing_area.grab_focus();
    }

    pub fn add_controller(&self, controller: &(impl IsA<gtk::EventController> + Clone)) {
        self.drawing_area.add_controller(controller.clone());
    }

    pub fn emit_copy_clipboard(&self) {}

    pub fn emit_paste_clipboard(&self) {}

    pub fn pty(&self) -> Option<std::os::unix::io::RawFd> {
        self.state.lock().unwrap().pty_fd
    }

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
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let working_dir = working_dir.map(str::to_owned);

        let (master_fd, slave_fd): (RawFd, RawFd) = unsafe {
            let mut master: RawFd = -1;
            let mut slave: RawFd = -1;
            let mut winsize = libc::winsize {
                ws_row: 24,
                ws_col: 80,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            if libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut winsize,
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

        let mut command = Command::new(&shell);
        if let Some(dir) = &working_dir {
            if !dir.is_empty() && std::path::Path::new(dir).is_dir() {
                command.current_dir(dir);
            } else if !dir.is_empty() {
                eprintln!("[terminal] working_dir '{dir}' is not a valid directory, ignoring");
            }
        }

        unsafe {
            command
                .stdin(Stdio::from_raw_fd(slave_fd))
                .stdout(Stdio::from_raw_fd(libc::dup(slave_fd)))
                .stderr(Stdio::from_raw_fd(libc::dup(slave_fd)))
                .pre_exec(move || {
                    libc::setsid();
                    libc::ioctl(0, libc::TIOCSCTTY as _, 0);
                    Ok(())
                });
        }

        match command.spawn() {
            Ok(mut child) => {
                let pid = glib::Pid(child.id() as i32);

                {
                    let mut state = self.state.lock().unwrap();
                    state.pty_fd = Some(master_fd);
                }

                let state_clone = self.state.clone();
                let draw_sender = self.draw_sender.clone();

                let handle = std::thread::spawn(move || {
                    let mut buf = [0u8; 4096];
                    let mut parser = Parser::new();
                    let mut handler = TerminalHandler {
                        state: state_clone.clone(),
                        draw_sender: draw_sender.clone(),
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
                unsafe { libc::close(master_fd) };
                callback(Err(glib::Error::new(
                    glib::FileError::Failed,
                    &e.to_string(),
                )));
            }
        }
    }
}

fn draw_terminal(area: &DrawingArea, cr: &Context, state: &TerminalState, width: i32, height: i32) {
    cr.set_source_rgba(
        state.bg_color.red() as f64,
        state.bg_color.green() as f64,
        state.bg_color.blue() as f64,
        state.bg_color.alpha() as f64,
    );
    cr.rectangle(0.0, 0.0, width as f64, height as f64);
    cr.fill().unwrap();

    let layout = area.create_pango_layout(None);
    layout.set_font_description(Some(&state.font_desc));

    layout.set_text("W");
    let extents = layout.pixel_extents();
    let char_width = extents.1.width() as f64;
    let char_height = extents.1.height() as f64;

    let total_scrollback = state.scrollback.len();
    let scroll_offset = state.scroll_offset;
    let visible_rows = (height as f64 / char_height).floor() as usize;

    let start_abs_row = if scroll_offset > 0 {
        total_scrollback.saturating_sub(scroll_offset)
    } else {
        total_scrollback
    };

    let mut abs_row = start_abs_row;
    let mut drawn = 0;

    while drawn < visible_rows && abs_row < total_scrollback + state.grid.len() {
        let row = if abs_row < total_scrollback {
            &state.scrollback[abs_row]
        } else {
            let grid_idx = abs_row - total_scrollback;
            if grid_idx < state.grid.len() {
                &state.grid[grid_idx]
            } else {
                break;
            }
        };

        let y_pos = drawn as f64 * char_height;

        let is_selected =
            if let (Some(start), Some(end)) = (state.selection_start, state.selection_end) {
                let row1 = start.0.min(end.0);
                let row2 = start.0.max(end.0);
                abs_row >= row1 && abs_row <= row2
            } else {
                false
            };

        for (col, cell) in row.iter().enumerate() {
            if cell.ch != ' ' {
                let x_pos = col as f64 * char_width;

                let cell_selected = if is_selected {
                    let (start_col, end_col) = if let (Some(start), Some(end)) =
                        (state.selection_start, state.selection_end)
                    {
                        let (row1, row2) = (start.0.min(end.0), start.0.max(end.0));
                        if abs_row == row1 && abs_row == row2 {
                            (start.1.min(end.1), start.1.max(end.1))
                        } else if abs_row == row1 {
                            (start.1, state.cols - 1)
                        } else if abs_row == row2 {
                            (0, end.1)
                        } else {
                            (0, state.cols - 1)
                        }
                    } else {
                        (0, 0)
                    };
                    col >= start_col && col <= end_col
                } else {
                    false
                };

                if cell_selected {
                    cr.set_source_rgba(0.3, 0.5, 0.9, 0.5);
                    cr.rectangle(x_pos, y_pos, char_width, char_height);
                    cr.fill().unwrap();
                    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                } else if let Some(fg) = &cell.fg {
                    cr.set_source_rgba(
                        fg.red() as f64,
                        fg.green() as f64,
                        fg.blue() as f64,
                        fg.alpha() as f64,
                    );
                } else {
                    cr.set_source_rgba(
                        state.fg_color.red() as f64,
                        state.fg_color.green() as f64,
                        state.fg_color.blue() as f64,
                        state.fg_color.alpha() as f64,
                    );
                }

                let mut text = String::new();
                text.push(cell.ch);
                layout.set_text(&text);
                cr.move_to(x_pos, y_pos);
                pangocairo::functions::show_layout(cr, &layout);
            }
        }

        drawn += 1;
        abs_row += 1;
    }

    if state.scroll_offset == 0 {
        let cursor_x = state.cursor_x;
        let cursor_y = state.cursor_y;
        if cursor_y < state.rows && cursor_x < state.cols {
            let x_pos = cursor_x as f64 * char_width;
            let y_pos = cursor_y as f64 * char_height;
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.3);
            cr.rectangle(x_pos, y_pos, char_width, char_height);
            cr.fill().unwrap();
        }
    }

    if state.scroll_offset > 0 {
        let indicator_text = format!("↑ {} lines", state.scroll_offset);
        layout.set_text(&indicator_text);
        let text_width = layout.pixel_extents().1.width() as f64;
        let x_pos = width as f64 - text_width - 10.0;
        let y_pos = 5.0;

        cr.set_source_rgba(0.0, 0.0, 0.0, 0.7);
        cr.rectangle(
            x_pos - 5.0,
            y_pos - 2.0,
            text_width + 10.0,
            char_height + 4.0,
        );
        cr.fill().unwrap();

        cr.set_source_rgba(0.8, 0.8, 0.8, 1.0);
        cr.move_to(x_pos, y_pos);
        pangocairo::functions::show_layout(cr, &layout);
    }
}
