//NOTE: add scrollbar

use crate::model::TerminalConfig;
use gtk::cairo::Context;
use gtk::gio;
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
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub reverse: bool,
}

impl Cell {
    /// Returns a blank space cell with no attributes.
    #[inline]
    pub fn blank() -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg: None,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            strikethrough: false,
            reverse: false,
        }
    }
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
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub reverse: bool,
    pub scrollback: Vec<Vec<Cell>>,
    pub scrollback_limit: usize,
    pub scroll_offset: usize,
    pub selection_start: Option<(usize, usize)>,
    pub selection_end: Option<(usize, usize)>,
    pub selection_active: bool,
    pub pty_master_fd: Option<RawFd>,
    /// Saved grid/cursor for the alternate screen buffer (\e[?1049h/l).
    pub alt_screen: Option<(Vec<Vec<Cell>>, usize, usize)>,
    pub bracketed_paste: bool,
    pub cursor_visible: bool,
    pub focus_reporting: bool,
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
                    dim: false,
                    italic: false,
                    underline: false,
                    strikethrough: false,
                    reverse: false,
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
            fg_color: fg,
            bg_color: bg,
            font_desc: pango::FontDescription::from_string("JetBrains Mono 13"),
            pty_fd: None,
            saved_cursor_x: 0,
            saved_cursor_y: 0,
            current_fg: fg,
            current_bg: bg,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            strikethrough: false,
            reverse: false,
            scrollback: Vec::new(),
            scrollback_limit: 10000,
            scroll_offset: 0,
            selection_start: None,
            selection_end: None,
            selection_active: false,
            pty_master_fd: None,
            alt_screen: None,
            bracketed_paste: false,
            cursor_visible: true,
            focus_reporting: false,
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
                        Cell::blank()
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
                    Cell::blank()
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

        if let Some(fd) = self.pty_master_fd {
            let winsize = libc::winsize {
                ws_row: rows as u16,
                ws_col: cols as u16,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            unsafe {
                libc::ioctl(fd, libc::TIOCSWINSZ, &winsize);
            }
        }
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

        let empty_row = vec![Cell::blank(); self.cols];
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
                *cell = Cell::blank();
            }
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.reset_attrs();
        self.scrollback.clear();
        self.scroll_offset = 0;
        self.selection_start = None;
        self.selection_end = None;
        self.selection_active = false;
    }

    fn reset_attrs(&mut self) {
        self.current_fg = self.fg_color;
        self.current_bg = self.bg_color;
        self.bold = false;
        self.dim = false;
        self.italic = false;
        self.underline = false;
        self.strikethrough = false;
        self.reverse = false;
    }

    fn blank_row(&self) -> Vec<Cell> {
        vec![Cell::blank(); self.cols]
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

    /// Switches to the alternate screen buffer, saving the current grid and cursor.
    pub fn enter_alt_screen(&mut self) {
        if self.alt_screen.is_some() {
            return;
        }
        self.alt_screen = Some((self.grid.clone(), self.cursor_x, self.cursor_y));
        self.grid = (0..self.rows).map(|_| self.blank_row()).collect();
        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    /// Restores the primary screen buffer saved by [`enter_alt_screen`].
    pub fn exit_alt_screen(&mut self) {
        if let Some((saved_grid, cx, cy)) = self.alt_screen.take() {
            self.grid = saved_grid;
            self.cursor_x = cx;
            self.cursor_y = cy;
        }
    }

    /// Expands a xterm 256-color palette index into an RGBA value.
    pub fn color_from_256(index: u16) -> gtk::gdk::RGBA {
        // First 16 are the standard ANSI colors (same table used in SGR 30-37/90-97).
        const ANSI16: [(f32, f32, f32); 16] = [
            (0.0, 0.0, 0.0),
            (0.8, 0.0, 0.0),
            (0.0, 0.8, 0.0),
            (0.8, 0.8, 0.0),
            (0.0, 0.0, 0.8),
            (0.8, 0.0, 0.8),
            (0.0, 0.8, 0.8),
            (0.8, 0.8, 0.8),
            (0.4, 0.4, 0.4),
            (1.0, 0.2, 0.2),
            (0.2, 1.0, 0.2),
            (1.0, 1.0, 0.2),
            (0.2, 0.2, 1.0),
            (1.0, 0.2, 1.0),
            (0.2, 1.0, 1.0),
            (1.0, 1.0, 1.0),
        ];
        if index < 16 {
            let (r, g, b) = ANSI16[index as usize];
            return gtk::gdk::RGBA::new(r, g, b, 1.0);
        }
        // 6x6x6 colour cube: indices 16-231.
        if index < 232 {
            let i = index - 16;
            let b = (i % 6) as f32;
            let g = ((i / 6) % 6) as f32;
            let r = (i / 36) as f32;
            let scale = |v: f32| {
                if v == 0.0 {
                    0.0
                } else {
                    (55.0 + v * 40.0) / 255.0
                }
            };
            return gtk::gdk::RGBA::new(scale(r), scale(g), scale(b), 1.0);
        }
        // Grayscale ramp: indices 232-255.
        let level = (8 + (index - 232) * 10) as f32 / 255.0;
        gtk::gdk::RGBA::new(level, level, level, 1.0)
    }

    pub fn send_cursor_position(&self) {
        let row = self.cursor_y + 1;
        let col = self.cursor_x + 1;
        let response = format!("\x1b[{};{}R", row, col);
        if let Some(fd) = self.pty_fd {
            unsafe {
                libc::write(fd, response.as_ptr() as *const libc::c_void, response.len());
            }
        }
    }

    pub fn send_background_color(&self) {
        let bg = &self.bg_color;
        let r = (bg.red() * 65535.0) as u16;
        let g = (bg.green() * 65535.0) as u16;
        let b = (bg.blue() * 65535.0) as u16;
        let response = format!("\x1b]11;rgb:{:04x}/{:04x}/{:04x}\x1b\\", r, g, b);
        if let Some(fd) = self.pty_fd {
            unsafe {
                libc::write(fd, response.as_ptr() as *const libc::c_void, response.len());
            }
        }
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
            let (fg, bg) = if state.reverse {
                (Some(state.current_bg), Some(state.current_fg))
            } else {
                (Some(state.current_fg), Some(state.current_bg))
            };
            state.grid[y][x] = Cell {
                ch: c,
                fg,
                bg,
                bold: state.bold,
                dim: state.dim,
                italic: state.italic,
                underline: state.underline,
                strikethrough: state.strikethrough,
                reverse: state.reverse,
            };
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
            b'\x08' if state.cursor_x > 0 => {
                state.cursor_x -= 1;
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

        let mut p: Vec<i64> = Vec::new();
        for param in params.iter() {
            if let Some(&val) = param.first() {
                p.push(val as i64);
            }
        }

        let has_question = intermediates.first().copied() == Some(b'?');
        let has_gt = intermediates.first().copied() == Some(b'>');

        match command {
            'c' => {
                let response: &[u8] = if has_gt {
                    b"\x1b[>0;0;0c"
                } else {
                    b"\x1b[?1;0c"
                };
                if let Some(fd) = state.pty_fd {
                    let _ = unsafe {
                        libc::write(fd, response.as_ptr() as *const libc::c_void, response.len())
                    };
                }
            }
            'A' => {
                let n = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                state.cursor_y = state.cursor_y.saturating_sub(n);
            }
            'B' => {
                let n = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                state.cursor_y = (state.cursor_y + n).min(state.rows - 1);
            }
            'C' => {
                let n = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                state.cursor_x = (state.cursor_x + n).min(state.cols - 1);
            }
            'D' => {
                let n = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                state.cursor_x = state.cursor_x.saturating_sub(n);
            }
            'E' => {
                let n = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                state.cursor_y = (state.cursor_y + n).min(state.rows - 1);
                state.cursor_x = 0;
            }
            'F' => {
                let n = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                state.cursor_y = state.cursor_y.saturating_sub(n);
                state.cursor_x = 0;
            }
            'G' => {
                let col = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                state.cursor_x = (col - 1).min(state.cols - 1);
            }
            'H' | 'f' => {
                let row = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                let col = p.get(1).map(|&v| v as usize).unwrap_or(1).max(1);
                state.cursor_y = (row - 1).min(state.rows - 1);
                state.cursor_x = (col - 1).min(state.cols - 1);
            }
            'J' => {
                let rows = state.rows;
                let cols = state.cols;
                let cy = state.cursor_y;
                let cx = state.cursor_x;
                match p.first().copied().unwrap_or(0) {
                    0 => {
                        for y in cy..rows {
                            for x in 0..cols {
                                if y == cy && x < cx {
                                    continue;
                                }
                                state.grid[y][x] = Cell::blank();
                            }
                        }
                    }
                    1 => {
                        for y in 0..=cy {
                            for x in 0..cols {
                                if y == cy && x > cx {
                                    continue;
                                }
                                state.grid[y][x] = Cell::blank();
                            }
                        }
                    }
                    2 | 3 => {
                        let rows = state.rows;
                        let cols = state.cols;
                        for y in 0..rows {
                            for x in 0..cols {
                                state.grid[y][x] = Cell::blank();
                            }
                        }
                        state.cursor_x = 0;
                        state.cursor_y = 0;
                    }
                    _ => {}
                }
            }
            'K' => {
                let row = state.cursor_y;
                let cx = state.cursor_x;
                let cols = state.cols;
                match p.first().copied().unwrap_or(0) {
                    0 => {
                        for x in cx..cols {
                            state.grid[row][x] = Cell::blank();
                        }
                    }
                    1 => {
                        for x in 0..=cx {
                            state.grid[row][x] = Cell::blank();
                        }
                    }
                    2 => {
                        for x in 0..cols {
                            state.grid[row][x] = Cell::blank();
                        }
                    }
                    _ => {}
                }
            }
            'L' => {
                // Insert Ps blank lines at cursor row, scrolling down.
                let n = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                let cy = state.cursor_y;
                let rows = state.rows;
                let cols = state.cols;
                for _ in 0..n {
                    if rows > 0 {
                        state.grid.pop();
                    }
                    state.grid.insert(cy, vec![Cell::blank(); cols]);
                }
            }
            'M' => {
                // Delete Ps lines at cursor row, scrolling up.
                let n = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                let cy = state.cursor_y;
                let rows = state.rows;
                let cols = state.cols;
                for _ in 0..n {
                    if cy < state.grid.len() {
                        state.grid.remove(cy);
                        state.grid.push(vec![Cell::blank(); cols]);
                    }
                }
                let _ = rows;
            }
            'P' => {
                // Delete Ps characters at cursor position.
                let n = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                let cy = state.cursor_y;
                let cx = state.cursor_x;
                let cols = state.cols;
                let row = &mut state.grid[cy];
                for _ in 0..n {
                    if cx < row.len() {
                        row.remove(cx);
                        row.push(Cell::blank());
                    }
                }
                let _ = cols;
            }
            'S' => {
                // Scroll up Ps lines (content moves up, new blank lines at bottom).
                let n = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                for _ in 0..n {
                    state.scroll_up();
                }
            }
            '@' => {
                // Insert Ps blank characters at cursor.
                let n = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                let cy = state.cursor_y;
                let cx = state.cursor_x;
                let cols = state.cols;
                let row = &mut state.grid[cy];
                for _ in 0..n {
                    if cx < row.len() {
                        row.insert(cx, Cell::blank());
                        row.truncate(cols);
                    }
                }
            }
            'd' => {
                // Move cursor to absolute row Ps (1-based).
                let row = p.first().map(|&v| v as usize).unwrap_or(1).max(1);
                state.cursor_y = (row - 1).min(state.rows - 1);
            }
            'm' => {
                // SGR - Select Graphic Rendition.
                if p.is_empty() {
                    state.reset_attrs();
                } else {
                    let mut i = 0;
                    while i < p.len() {
                        match p[i] {
                            0 => state.reset_attrs(),
                            1 => state.bold = true,
                            2 => state.dim = true,
                            3 => state.italic = true,
                            4 => state.underline = true,
                            7 => state.reverse = true,
                            9 => state.strikethrough = true,
                            22 => {
                                state.bold = false;
                                state.dim = false;
                            }
                            23 => state.italic = false,
                            24 => state.underline = false,
                            27 => state.reverse = false,
                            29 => state.strikethrough = false,
                            // Standard foreground colors (30-37).
                            30..=37 => {
                                state.current_fg = ansi_color(p[i] as u16 - 30, false);
                            }
                            // Extended foreground: 38,5,Ps (256-color) or 38,2,r,g,b (true-color).
                            38 => match p.get(i + 1).copied() {
                                Some(5) if p.len() > i + 2 => {
                                    state.current_fg =
                                        TerminalState::color_from_256(p[i + 2] as u16);
                                    i += 2;
                                }
                                Some(2) if p.len() > i + 4 => {
                                    state.current_fg = rgb_color(p[i + 2], p[i + 3], p[i + 4]);
                                    i += 4;
                                }
                                _ => {}
                            },
                            // Reset foreground to default.
                            39 => state.current_fg = state.fg_color,
                            // Standard background colors (40-47).
                            40..=47 => {
                                state.current_bg = ansi_color(p[i] as u16 - 40, false);
                            }
                            // Extended background: 48,5,Ps or 48,2,r,g,b.
                            48 => match p.get(i + 1).copied() {
                                Some(5) if p.len() > i + 2 => {
                                    state.current_bg =
                                        TerminalState::color_from_256(p[i + 2] as u16);
                                    i += 2;
                                }
                                Some(2) if p.len() > i + 4 => {
                                    state.current_bg = rgb_color(p[i + 2], p[i + 3], p[i + 4]);
                                    i += 4;
                                }
                                _ => {}
                            },
                            // Reset background to default.
                            49 => state.current_bg = state.bg_color,
                            // Bright foreground colors (90-97).
                            90..=97 => {
                                state.current_fg = ansi_color(p[i] as u16 - 90, true);
                            }
                            // Bright background colors (100-107).
                            100..=107 => {
                                state.current_bg = ansi_color(p[i] as u16 - 100, true);
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                }
            }
            'n' => {
                // Device Status Report - \e[6n requests cursor position (no ? prefix).
                if !has_question && p.first().copied().unwrap_or(0) == 6 {
                    state.send_cursor_position();
                }
            }
            'h' | 'l' if has_question => {
                let enable = command == 'h';
                for &mode in &p {
                    match mode {
                        25 => state.cursor_visible = enable,
                        1004 => state.focus_reporting = enable,
                        1049 => {
                            if enable {
                                state.enter_alt_screen();
                            } else {
                                state.exit_alt_screen();
                            }
                        }
                        2004 => state.bracketed_paste = enable,
                        // Modes the terminal acknowledges but doesn't act on (ignored).
                        _ => {}
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
            // Ignore unknown sequences per ECMA-48.
            _ => {}
        }
        let _ = self.draw_sender.send(());
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _command: bool) {
        let state = self.state.lock().unwrap();

        if params.is_empty() {
            return;
        }

        let cmd = std::str::from_utf8(params[0])
            .unwrap_or("")
            .parse::<u16>()
            .unwrap_or(u16::MAX);

        let payload = if params.len() > 1 {
            params[1..].concat()
        } else {
            Vec::new()
        };
        let payload_str = String::from_utf8_lossy(&payload);

        match cmd {
            // OSC 0 / 1 - Set window/tab title. fish_title and fish_tab_title use these.
            0 | 1 => {
                // Title updates are intentionally not stored: expose via a callback if
                // the embedding widget ever needs to forward them to a notebook tab label.
            }
            // OSC 7 - Report working directory (file://hostname/path).
            7 => {
                // Available for the parent widget to read via TerminalState if desired.
                // No action required for fish to function correctly.
            }
            // OSC 8 - Hyperlinks. Silently ignored, fish uses them for man pages.
            8 => {}
            // OSC 11 - Query background color. fish sends \e]11,?\e\\.
            11 if payload_str == "?" => {
                state.send_background_color();
            }
            // OSC 52 - Clipboard copy. fish_clipboard_copy uses this.
            // OSC 52 - Clipboard write. Requires a GDK display handle unavailable
            // from the PTY reader thread, wire up via draw channel if needed.
            52 => {}
            // OSC 133 - Shell integration marks (prompt/command start/end). Silently
            // accepted so fish doesn't stall waiting for a negative acknowledgment.
            133 => {}
            _ => {}
        }
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
        drawing_area.set_height_request(char_height * config.height);

        let (draw_sender, mut draw_receiver) = tokio::sync::mpsc::unbounded_channel::<()>();

        let drawing_area_clone = drawing_area.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Some(()) = draw_receiver.recv().await {
                drawing_area_clone.queue_draw();
            }
        });

        let state = Arc::new(Mutex::new(TerminalState::new(80, 24)));
        state.lock().unwrap().font_desc = font_desc;

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

                if new_cols > 0
                    && new_rows > 0
                    && (new_cols != state.cols || new_rows != state.rows)
                {
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

            if is_ctrl && is_shift && (keyval == gtk::gdk::Key::v || keyval == gtk::gdk::Key::V) {
                let state_clone = state_for_keys.clone();
                if let Some(window) = drawing_area_for_keys.root() {
                    let display = gtk::prelude::RootExt::display(&window);
                    let clipboard = display.clipboard();
                    clipboard.read_text_async(gio::Cancellable::NONE, move |result| {
                        if let Ok(Some(text)) = result {
                            let state = state_clone.lock().unwrap();
                            if let Some(fd) = state.pty_fd {
                                let write = |data: &[u8]| unsafe {
                                    libc::write(
                                        fd,
                                        data.as_ptr() as *const libc::c_void,
                                        data.len(),
                                    );
                                };
                                if state.bracketed_paste {
                                    write(b"\x1b[200~");
                                }
                                write(text.as_bytes());
                                if state.bracketed_paste {
                                    write(b"\x1b[201~");
                                }
                            }
                        }
                    });
                }
                return glib::Propagation::Stop;
            }

            match keyval {
                gtk::gdk::Key::Page_Up if is_shift => {
                    let mut state = state_for_keys.lock().unwrap();
                    state.scroll_lines(20);
                    drawing_area_for_keys.queue_draw();
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Page_Down if is_shift => {
                    let mut state = state_for_keys.lock().unwrap();
                    state.scroll_lines(-20);
                    drawing_area_for_keys.queue_draw();
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::BackSpace => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ =
                            unsafe { libc::write(fd, b"\x7f".as_ptr() as *const libc::c_void, 1) };
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Return => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ =
                            unsafe { libc::write(fd, b"\r".as_ptr() as *const libc::c_void, 1) };
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Tab => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ =
                            unsafe { libc::write(fd, b"\t".as_ptr() as *const libc::c_void, 1) };
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::c | gtk::gdk::Key::C if is_ctrl => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ =
                            unsafe { libc::write(fd, b"\x03".as_ptr() as *const libc::c_void, 1) };
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::d | gtk::gdk::Key::D if is_ctrl => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ =
                            unsafe { libc::write(fd, b"\x04".as_ptr() as *const libc::c_void, 1) };
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Up => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ = unsafe {
                            libc::write(fd, b"\x1b[A".as_ptr() as *const libc::c_void, 3)
                        };
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Down => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ = unsafe {
                            libc::write(fd, b"\x1b[B".as_ptr() as *const libc::c_void, 3)
                        };
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Left => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ = unsafe {
                            libc::write(fd, b"\x1b[D".as_ptr() as *const libc::c_void, 3)
                        };
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Right => {
                    if let Some(fd) = state_for_keys.lock().unwrap().pty_fd {
                        let _ = unsafe {
                            libc::write(fd, b"\x1b[C".as_ptr() as *const libc::c_void, 3)
                        };
                    }
                    glib::Propagation::Stop
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
                            glib::Propagation::Stop
                        } else {
                            glib::Propagation::Proceed
                        }
                    } else {
                        glib::Propagation::Proceed
                    }
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

        let term = Self {
            drawing_area,
            state,
            _pty_reader: None,
            draw_sender,
        };

        let term_clone = term.clone();
        term.drawing_area.connect_realize(move |_area| {
            let mut t = term_clone.clone();
            glib::idle_add_local_once(move || {
                t.spawn_async(
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
            });
        });

        term
    }

    pub fn feed_child(&self, data: &[u8]) {
        let state = self.state.lock().unwrap();
        state.write_pty(data);
    }

    pub fn set_color_foreground(&self, color: &gtk::gdk::RGBA) {
        let mut state = self.state.lock().unwrap();
        state.fg_color = *color;
        state.current_fg = *color;
        self.drawing_area.queue_draw();
    }

    pub fn set_color_background(&self, color: &gtk::gdk::RGBA) {
        let mut state = self.state.lock().unwrap();
        state.bg_color = *color;
        state.current_bg = *color;
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

    pub fn emit_copy_clipboard(&self) {
        let text = self.state.lock().unwrap().get_selected_text();
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

    pub fn pty(&self) -> Option<std::os::unix::io::RawFd> {
        self.state.lock().unwrap().pty_fd
    }

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
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let working_dir = working_dir.map(str::to_owned);

        let (width, height) = (self.drawing_area.width(), self.drawing_area.height());

        let layout = self.drawing_area.create_pango_layout(None);
        layout.set_font_description(Some(&self.state.lock().unwrap().font_desc));
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

        {
            let mut state = self.state.lock().unwrap();
            state.pty_master_fd = Some(master_fd);
            state.cols = cols;
            state.rows = rows;
        }

        let mut command = Command::new(&shell);
        command.env("TERM", "xterm-256color");

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

/// Maps a 3-bit ANSI color index (0-7) to an RGBA value.
/// `bright` selects the high-intensity variant used by SGR 90-97 / 100-107.
#[inline]
fn ansi_color(index: u16, bright: bool) -> gtk::gdk::RGBA {
    const NORMAL: [(f32, f32, f32); 8] = [
        (0.0, 0.0, 0.0),
        (0.8, 0.0, 0.0),
        (0.0, 0.8, 0.0),
        (0.8, 0.8, 0.0),
        (0.0, 0.0, 0.8),
        (0.8, 0.0, 0.8),
        (0.0, 0.8, 0.8),
        (0.8, 0.8, 0.8),
    ];
    const BRIGHT: [(f32, f32, f32); 8] = [
        (0.4, 0.4, 0.4),
        (1.0, 0.2, 0.2),
        (0.2, 1.0, 0.2),
        (1.0, 1.0, 0.2),
        (0.2, 0.2, 1.0),
        (1.0, 0.2, 1.0),
        (0.2, 1.0, 1.0),
        (1.0, 1.0, 1.0),
    ];
    let table = if bright { &BRIGHT } else { &NORMAL };
    let (r, g, b) = table[(index as usize).min(7)];
    gtk::gdk::RGBA::new(r, g, b, 1.0)
}

/// Converts a 24-bit RGB triple (0-255 each, passed as i64) into RGBA.
#[inline]
fn rgb_color(r: i64, g: i64, b: i64) -> gtk::gdk::RGBA {
    gtk::gdk::RGBA::new(
        (r as f32 / 255.0).clamp(0.0, 1.0),
        (g as f32 / 255.0).clamp(0.0, 1.0),
        (b as f32 / 255.0).clamp(0.0, 1.0),
        1.0,
    )
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

        let mut x_pos = 0.0;
        let mut col = 0;

        while col < row.len() {
            let cell = &row[col];

            if cell.ch == ' ' && !cell.underline && cell.bg.is_none() {
                col += 1;
                x_pos += char_width;
                continue;
            }

            // Collect a run of cells that share the same visual attributes so they
            // can be rendered as a single Pango layout call.
            let run_start_col = col;
            let first_cell = cell.clone();
            let mut text = String::new();

            while col < row.len() {
                let c = &row[col];
                let same_attrs = c.bold == first_cell.bold
                    && c.dim == first_cell.dim
                    && c.italic == first_cell.italic
                    && c.underline == first_cell.underline
                    && c.strikethrough == first_cell.strikethrough
                    && c.fg == first_cell.fg
                    && c.bg == first_cell.bg;
                if !same_attrs {
                    break;
                }
                text.push(c.ch);
                col += 1;
            }

            let mut font_desc = state.font_desc.clone();
            if first_cell.bold {
                font_desc.set_weight(pango::Weight::Bold);
            }
            if first_cell.italic {
                font_desc.set_style(pango::Style::Italic);
            }
            layout.set_font_description(Some(&font_desc));
            layout.set_text(&text);

            let attr_list = pango::AttrList::new();
            let byte_len = text.len() as u32;

            if first_cell.underline {
                let mut a = pango::AttrInt::new_underline(pango::Underline::Single);
                a.set_start_index(0);
                a.set_end_index(byte_len);
                attr_list.insert(a);
            }
            if first_cell.strikethrough {
                let mut a = pango::AttrInt::new_strikethrough(true);
                a.set_start_index(0);
                a.set_end_index(byte_len);
                attr_list.insert(a);
            }
            if first_cell.dim {
                // Dim = ~60% opacity on the foreground, approximate with alpha via color.
                let fg = first_cell.fg.as_ref().unwrap_or(&state.fg_color);
                let mut a = pango::AttrColor::new_foreground(
                    (fg.red() * 0.6 * 65535.0) as u16,
                    (fg.green() * 0.6 * 65535.0) as u16,
                    (fg.blue() * 0.6 * 65535.0) as u16,
                );
                a.set_start_index(0);
                a.set_end_index(byte_len);
                attr_list.insert(a);
            }
            layout.set_attributes(Some(&attr_list));

            let text_extents = layout.pixel_extents();
            let text_width = text_extents.1.width() as f64;
            let run_width = (col - run_start_col) as f64 * char_width;

            // Draw background cell fill.
            let effective_bg = if first_cell.reverse {
                first_cell.fg.as_ref().unwrap_or(&state.fg_color)
            } else {
                first_cell.bg.as_ref().unwrap_or(&state.bg_color)
            };
            if *effective_bg != state.bg_color {
                cr.set_source_rgba(
                    effective_bg.red() as f64,
                    effective_bg.green() as f64,
                    effective_bg.blue() as f64,
                    effective_bg.alpha() as f64,
                );
                cr.rectangle(x_pos, y_pos, run_width, char_height);
                cr.fill().unwrap();
            }

            let block_selected = if is_selected {
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
                let block_end = run_start_col + text.len();
                !(block_end <= start_col || run_start_col >= end_col)
            } else {
                false
            };

            if block_selected {
                cr.set_source_rgba(0.3, 0.5, 0.9, 0.5);
                cr.rectangle(x_pos, y_pos, run_width, char_height);
                cr.fill().unwrap();
                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
            } else if !first_cell.dim {
                let fg = if first_cell.reverse {
                    first_cell.bg.as_ref().unwrap_or(&state.bg_color)
                } else {
                    first_cell.fg.as_ref().unwrap_or(&state.fg_color)
                };
                cr.set_source_rgba(
                    fg.red() as f64,
                    fg.green() as f64,
                    fg.blue() as f64,
                    fg.alpha() as f64,
                );
            }

            if text.trim().is_empty() && !first_cell.underline {
                x_pos += run_width;
                layout.set_font_description(Some(&state.font_desc));
                layout.set_attributes(None);
                continue;
            }

            cr.move_to(x_pos, y_pos);
            pangocairo::functions::show_layout(cr, &layout);
            x_pos += text_width.max(run_width);

            layout.set_font_description(Some(&state.font_desc));
            layout.set_attributes(None);
        }

        drawn += 1;
        abs_row += 1;
    }

    if state.scroll_offset == 0 && state.cursor_visible {
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
