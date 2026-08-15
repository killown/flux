use crate::model::TerminalConfig;
use adw;
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

/// Returns `true` when the PTY slave is in raw or cbreak mode (`ICANON` clear).
///
/// Terminal emulators use this to decide whether to apply readline-style key
/// bindings (cooked mode) or to pass every byte straight through to the
/// application (raw mode). nvim, nano, less, and similar full-screen apps all
/// put the PTY in raw/cbreak mode while running.
///
/// A single `tcgetattr` syscall per keypress (~1 µs) is negligible.
#[inline]
fn pty_is_raw(fd: libc::c_int) -> bool {
    let mut termios = unsafe { std::mem::zeroed::<libc::termios>() };
    let ret = unsafe { libc::tcgetattr(fd, &mut termios) };
    if ret != 0 {
        return false;
    }
    termios.c_lflag & libc::ICANON == 0
}

pub struct TerminalState {
    pub cleaned_up: bool,
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
    /// PID of the shell process, used to detect whether the terminal is idle.
    pub shell_pid: Option<libc::pid_t>,
    /// Saved grid/cursor for the alternate screen buffer (\e[?1049h/l).
    pub alt_screen: Option<(Vec<Vec<Cell>>, usize, usize)>,
    pub bracketed_paste: bool,
    pub cursor_visible: bool,
    pub focus_reporting: bool,
    /// When true, the next printed character wraps to the next line first.
    /// Matches DECAWM: printing to the last column sets this flag instead of
    /// immediately advancing cursor_y, so a bare \r cancels the wrap without
    /// a spurious line increment.
    pub pending_wrap: bool,
    /// Cached character cell height in pixels, updated each draw cycle.
    pub char_height: f64,
    /// DECCKM: when true, arrow keys send application sequences (\eOA)
    /// instead of cursor sequences (\e[A). Required for nvim/vim navigation.
    pub application_cursor_keys: bool,
    /// Set when the shell spawned with fallback cols=80 (widget was hidden).
    /// Cleared after the first real resize sends a corrective SIGWINCH.
    pub needs_initial_sigwinch: bool,
    /// Accent color resolved from the active GTK theme (`@accent_bg_color`).
    /// Used for the cursor and selection highlight. `None` until `apply_theme`
    /// is called.
    pub accent_color: Option<gtk::gdk::RGBA>,
    /// The 16-entry ANSI color palette (indices 0-7 normal, 8-15 bright).
    ///
    /// Resolved from the active GTK/libadwaita named palette colors in
    /// [`apply_theme`] so that directory highlighting and other SGR colors
    /// follow the user's theme. Falls back to the xterm defaults when theme
    /// variables are unavailable.
    pub ansi_palette: [gtk::gdk::RGBA; 16],
    /// Invoked on the PTY reader thread whenever fish emits an OSC 7
    /// working-directory notification. The callback receives the decoded
    /// absolute path of the new directory.
    pub on_cwd_change: Option<Box<dyn Fn(std::path::PathBuf) + Send>>,
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
            cleaned_up: false,
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
            shell_pid: None,
            alt_screen: None,
            bracketed_paste: false,
            cursor_visible: true,
            focus_reporting: false,
            pending_wrap: false,
            char_height: 0.0,
            application_cursor_keys: false,
            needs_initial_sigwinch: false,
            accent_color: None,
            ansi_palette: default_ansi_palette(),
            on_cwd_change: None,
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
                let cell = if y < self.grid.len() && x < self.grid[y].len() {
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
        self.application_cursor_keys = false;
    }

    /// Expands a xterm 256-color palette index into an RGBA value.
    pub fn color_from_256(index: u16) -> gtk::gdk::RGBA {
        // First 16 delegate to the shared fallback palette (same entries used
        // for SGR 30-37/90-97 before theme resolution).
        if index < 16 {
            return default_ansi_palette()[index as usize];
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

    /// Returns `true` when no foreground process other than the shell itself
    /// is running in the PTY, i.e. it is safe to respawn without killing a
    /// user process.
    ///
    /// Uses `TIOCGPGRP` to read the foreground process group of the PTY master
    /// and compares it against the shell's own PID. If the foreground pgrp
    /// differs, a child process (e.g. `vim`, `htop`, a long compile) is active.
    pub fn is_idle(&self) -> bool {
        let (Some(master_fd), Some(shell_pid)) = (self.pty_master_fd, self.shell_pid) else {
            return true;
        };
        let mut fgpgrp: libc::pid_t = -1;
        let ret = unsafe { libc::ioctl(master_fd, libc::TIOCGPGRP, &mut fgpgrp) };
        if ret != 0 {
            return true;
        }
        fgpgrp == shell_pid
    }
}

impl std::fmt::Debug for TerminalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalState")
            .field("cols", &self.cols)
            .field("rows", &self.rows)
            .field("cursor_x", &self.cursor_x)
            .field("cursor_y", &self.cursor_y)
            .finish_non_exhaustive()
    }
}

pub struct TerminalHandler {
    pub state: Arc<Mutex<TerminalState>>,
    pub draw_sender: tokio::sync::mpsc::UnboundedSender<()>,
}

impl Perform for TerminalHandler {
    fn print(&mut self, c: char) {
        let mut state = self.state.lock().unwrap();

        // DECAWM pending wrap (xenl): if the previous character landed on the
        // last column, wrap NOW before printing the new character. This means
        // a bare \r after filling a line cancels the wrap, which fish requires.
        if state.pending_wrap {
            state.pending_wrap = false;
            state.cursor_x = 0;
            state.cursor_y += 1;
            if state.cursor_y >= state.rows {
                state.scroll_up();
                state.cursor_y = state.rows - 1;
            }
        }

        let y = state.cursor_y;
        let x = state.cursor_x;
        if y < state.grid.len() && x < state.grid[y].len() {
            // Only store an explicit bg when it differs from the terminal default.
            // Cells with bg=None are skipped in the draw loop, which prevents the
            // terminal background colour from overwriting nvim/vim colour schemes
            // that rely on the default background being transparent.
            let explicit_bg = if state.current_bg == state.bg_color {
                None
            } else {
                Some(state.current_bg)
            };
            let (fg, bg) = if state.reverse {
                (explicit_bg, Some(state.current_fg))
            } else {
                (Some(state.current_fg), explicit_bg)
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
            // Don't wrap yet - set the pending flag. The wrap happens at the
            // start of the next print(), so \r can still reset cursor_x first.
            state.cursor_x = state.cols - 1;
            state.pending_wrap = true;
        }
        state.selection_active = false;
        state.selection_start = None;
        state.selection_end = None;
        let _ = self.draw_sender.send(());
    }

    fn execute(&mut self, byte: u8) {
        let mut state = self.state.lock().unwrap();
        match byte {
            b'\r' => {
                state.cursor_x = 0;
                state.pending_wrap = false;
            }
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

        state.pending_wrap = false;

        match command {
            // \e[0c or \e[>0c are DA queries from the shell.
            // \e[?1,0c is our own response echoed back, ignore it.
            'c' if !has_question => {
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
            'c' => {} // \e[?...c is our own DA response echoed back, ignore
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
                let cols = state.cols.min(state.grid.first().map_or(0, |r| r.len()));
                let rows = state.rows.min(state.grid.len());
                let cy = state.cursor_y.min(rows.saturating_sub(1));
                let cx = state.cursor_x.min(cols.saturating_sub(1));
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
                    2 => {
                        // Push every non-blank grid row into scrollback so the
                        // user can still scroll up to see previous output, then
                        // blank the grid and home the cursor. This matches the
                        // behaviour of xterm / alacritty for `clear`.
                        let old_grid: Vec<Vec<Cell>> = std::mem::replace(
                            &mut state.grid,
                            (0..rows).map(|_| vec![Cell::blank(); cols]).collect(),
                        );
                        for row in old_grid {
                            if row.iter().any(|c| c.ch != ' ' || c.bg.is_some()) {
                                state.scrollback.push(row);
                                if state.scrollback.len() > state.scrollback_limit {
                                    state.scrollback.remove(0);
                                }
                            }
                        }
                        state.cursor_x = 0;
                        state.cursor_y = 0;
                        state.pending_wrap = false;
                    }
                    3 => {
                        // Erase scrollback and blank the grid (Ps=3 extension).
                        state.scrollback.clear();
                        state.scroll_offset = 0;
                        state.grid = (0..rows).map(|_| vec![Cell::blank(); cols]).collect();
                        state.cursor_x = 0;
                        state.cursor_y = 0;
                        state.pending_wrap = false;
                    }
                    _ => {}
                }
            }
            'K' => {
                let row = state.cursor_y;
                let grid_cols = state.grid.get(row).map_or(0, |r| r.len());
                let cx = state.cursor_x.min(grid_cols.saturating_sub(1));
                let cols = state.cols.min(grid_cols);
                if row < state.grid.len() {
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
                                state.current_fg = state.ansi_palette[(p[i] as usize - 30).min(7)];
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
                                state.current_bg = state.ansi_palette[(p[i] as usize - 40).min(7)];
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
                                state.current_fg =
                                    state.ansi_palette[8 + (p[i] as usize - 90).min(7)];
                            }
                            // Bright background colors (100-107).
                            100..=107 => {
                                state.current_bg =
                                    state.ansi_palette[8 + (p[i] as usize - 100).min(7)];
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                }
            }
            'n' if !has_question && p.first().copied().unwrap_or(0) == 6 => {
                // Device Status Report - \e[6n requests cursor position (no ? prefix).
                state.send_cursor_position();
            }
            'h' | 'l' if has_question => {
                let enable = command == 'h';
                for &mode in &p {
                    match mode {
                        1 => state.application_cursor_keys = enable,
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
            // fish emits this on every prompt, used to sync the file manager's
            // navigation pane without the user typing anything.
            7 => {
                let path_str = payload_str
                    .strip_prefix("file://")
                    .map(|s| {
                        // Strip optional hostname: "file://host/path" → "/path"
                        //                          "file:///path"     → "/path"
                        s.find('/').map(|i| &s[i..]).unwrap_or(s)
                    })
                    .unwrap_or(&payload_str);

                let path = std::path::PathBuf::from(percent_decode(path_str));
                if path.is_dir() {
                    if let Some(cb) = &state.on_cwd_change {
                        cb(path);
                    }
                }
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
    /// Last directory queued for respawn, only the most recent survives rapid navigation.
    pending_dir: Arc<Mutex<Option<String>>>,
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
            pending_dir: self.pending_dir.clone(),
        }
    }
}

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
        let key_controller = gtk::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        key_controller.connect_key_pressed(move |_ctrl, keyval, _keycode, modifiers| {
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
                gtk::gdk::Key::Return => {
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
                            if ch.is_ascii_graphic() || ch == ' ' {
                                let mut seq = [0u8; 5];
                                seq[0] = 0x1b;
                                let mut tmp = [0u8; 4];
                                let n = ch.encode_utf8(&mut tmp).len();
                                seq[1..1 + n].copy_from_slice(&tmp[..n]);
                                pty_write(fd, &seq[..1 + n]);
                                return glib::Propagation::Stop;
                            }
                        }
                    }
                    if let Some(ch) = keyval.to_unicode() {
                        if ch.is_ascii_graphic() || ch == ' ' {
                            let mut buf = [0u8; 4];
                            let bytes = ch.encode_utf8(&mut buf);
                            pty_write(fd, bytes.as_bytes());
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

        let term = Self {
            drawing_area,
            state,
            _pty_reader: None,
            draw_sender,
            pending_dir: Arc::new(Mutex::new(None)),
        };

        // Spawn the shell on the first size-allocate with non-zero dimensions
        // rather than on realize, because the terminal pane is hidden at startup
        // so realize fires with width=height=0, causing fish to start with the
        // fallback 80x24 size. connect_size_allocate fires once the pane is
        // actually shown and GTK has assigned real pixel dimensions.
        let term_clone = term.clone();
        let config_height_lines = config.height;

        term.drawing_area.connect_map(move |area| {
            let area_clone = area.clone();
            let state_clone = term_clone.state.clone();

            // Defer paned position adjustment until GTK completes layout allocation for the panel
            glib::idle_add_local_once(move || {
                if let Some(paned) = area_clone
                    .ancestor(gtk::Paned::static_type())
                    .and_then(|w| w.downcast::<gtk::Paned>().ok())
                {
                    let state_for_check = state_clone.clone();
                    let set_position_if_allocated = move |p: &gtk::Paned| -> bool {
                        let total_h = p.height();
                        if total_h > 0 {
                            let ch = {
                                let s = state_for_check.lock().unwrap();
                                if s.char_height > 0.0 {
                                    s.char_height as i32
                                } else {
                                    char_height
                                }
                            };
                            let desired_pixel_height = config_height_lines * ch;
                            p.set_position(total_h - desired_pixel_height);
                            true
                        } else {
                            false
                        }
                    };

                    if !set_position_if_allocated(&paned) {
                        let state_retry = state_clone.clone();
                        let paned_retry = paned.clone();
                        glib::idle_add_local_once(move || {
                            let total_h = paned_retry.height();
                            if total_h > 0 {
                                let ch = {
                                    let s = state_retry.lock().unwrap();
                                    if s.char_height > 0.0 {
                                        s.char_height as i32
                                    } else {
                                        char_height
                                    }
                                };
                                paned_retry.set_position(total_h - config_height_lines * ch);
                            }
                        });
                    }
                }
            });
        });
        term
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

    /// Registers a callback invoked on the PTY reader thread whenever fish
    /// reports a working-directory change via OSC 7.
    ///
    /// The callback receives the decoded, absolute path of the new directory.
    /// Register this before the widget is realized to avoid missing the first
    /// prompt. The callback must be `Send` because it is called from the PTY
    /// reader thread, use a channel (e.g. `relm4::Sender`) rather than touching
    /// GTK objects directly.
    ///
    /// # Example
    ///
    /// ```ignore
    /// terminal.set_cwd_callback({
    ///     let sender = app_sender.clone(),
    ///     move |path| {
    ///         let _ = sender.send(AppMsg::NavigateTo(path)),
    ///     }
    /// }),
    /// ```
    /// Resolves terminal colors and font from the active GTK/libadwaita theme,
    /// then applies them, but only for config fields that are empty/default,
    /// preserving any explicit user overrides in `config.toml`.
    ///
    /// Call this once after construction and again whenever the system theme
    /// changes (see [`connect_theme_changes`]).
    ///
    /// Resolution order per field (first non-empty wins):
    /// - **fg_color**: config hex → `@theme_fg_color` → fallback `#E5E5E5 / #1A1A1A`
    /// - **bg_color**: config hex → `@window_bg_color` → fallback `#1A1A1A / #FAFAFA`
    /// - **font**: config string (non-default) → system monospace → `"monospace 13"`
    pub fn apply_theme(&self, config: &TerminalConfig) {
        let widget = self.drawing_area.upcast_ref::<gtk::Widget>();

        // --- colors -------------------------------------------------------
        let style = widget.style_context();

        let theme_fg = style.lookup_color("theme_fg_color");
        let window_bg = style.lookup_color("window_bg_color");
        let accent = style.lookup_color("accent_bg_color");

        // fg: config hex → GTK theme_fg_color → hardcoded fallback
        let fg = if !config.fg_color.is_empty() {
            config.fg_color.parse::<gtk::gdk::RGBA>().ok()
        } else {
            None
        }
        .or(theme_fg)
        .unwrap_or_else(|| {
            let dark = adw::StyleManager::default().is_dark();
            if dark {
                gtk::gdk::RGBA::new(0.898, 0.898, 0.898, 1.0)
            } else {
                gtk::gdk::RGBA::new(0.133, 0.133, 0.133, 1.0)
            }
        });

        // bg: config hex → GTK window_bg_color → hardcoded fallback
        let bg = if !config.bg_color.is_empty() {
            config.bg_color.parse::<gtk::gdk::RGBA>().ok()
        } else {
            None
        }
        .or(window_bg)
        .unwrap_or_else(|| {
            let dark = adw::StyleManager::default().is_dark();
            if dark {
                gtk::gdk::RGBA::new(0.102, 0.106, 0.149, 1.0)
            } else {
                gtk::gdk::RGBA::new(0.980, 0.980, 0.980, 1.0)
            }
        });

        self.set_color_foreground(&fg);
        self.set_color_background(&bg);

        // Resolve the 16-color ANSI palette from the libadwaita named palette.
        // Each GTK variable maps to an xterm ANSI slot, missing variables fall
        // back to the compiled-in xterm defaults for that slot only.
        let palette_vars: [(&str, usize); 14] = [
            ("green_3", 2),
            ("green_5", 10),
            ("yellow_3", 3),
            ("yellow_5", 11),
            ("blue_3", 4),
            ("blue_5", 12),
            ("purple_3", 5),
            ("purple_5", 13),
            ("cyan", 6),
            ("blue_4", 14),
            ("red_3", 1),
            ("red_5", 9),
            ("dark_2", 8),
            ("light_5", 15),
        ];
        let mut palette = default_ansi_palette();
        for (var, slot) in palette_vars {
            if let Some(c) = style.lookup_color(var) {
                palette[slot] = c;
            }
        }

        {
            let mut state = self.state.lock().unwrap();
            state.ansi_palette = palette;
            // cursor / selection tint from accent color
            if let Some(acc) = accent {
                state.accent_color = Some(acc);
            }
        }

        // --- font ---------------------------------------------------------
        // Only substitute the theme monospace font when the config value is
        // the compiled-in default (user hasn't customised it).
        const DEFAULT_FONT: &str = "JetBrains Mono 13";
        let font_str = if config.font.is_empty() || config.font == DEFAULT_FONT {
            // Ask GTK settings for the system monospace font, then append the
            // size from the default so it looks reasonable out of the box.
            gtk::Settings::default()
                .and_then(|s| s.gtk_font_name())
                .map(|_| {
                    // Use "Geist Mono" from the Flux CSS if available,
                    // otherwise fall back to the GTK monospace font.
                    let families = ["Geist Mono", "JetBrains Mono", "monospace"];
                    let pango_ctx = widget.pango_context();
                    let available: Vec<String> = pango_ctx
                        .font_map()
                        .map(|fm| {
                            fm.list_families()
                                .iter()
                                .map(|f| f.name().to_string())
                                .collect()
                        })
                        .unwrap_or_default();
                    let chosen = families
                        .iter()
                        .find(|&&f| available.iter().any(|a| a == f))
                        .copied()
                        .unwrap_or("monospace");
                    format!("{} 13", chosen)
                })
                .unwrap_or_else(|| DEFAULT_FONT.to_string())
        } else {
            config.font.clone()
        };

        self.set_font(Some(&pango::FontDescription::from_string(&font_str)));
    }

    /// Connects to `adw::StyleManager::dark` property changes so the terminal
    /// re-applies the resolved theme whenever the user switches between light
    /// and dark mode at runtime. Only color fields that are empty in `config`
    /// will be updated, user-overridden values are preserved.
    pub fn connect_theme_changes(&self, config: TerminalConfig) {
        let term = self.clone();
        adw::StyleManager::default().connect_dark_notify(move |_| {
            term.apply_theme(&config);
        });
    }

    pub fn set_cwd_callback<F>(&self, f: F)
    where
        F: Fn(std::path::PathBuf) + Send + 'static,
    {
        self.state.lock().unwrap().on_cwd_change = Some(Box::new(f));
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
        let state = self.state.lock().unwrap();
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

    #[allow(dead_code)]
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
        let spawned_hidden = width == 0 || height == 0;

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

        // Disable echo on the master side only, the shell handles its own
        // echoing. Preserving ONLCR (NL→CR+NL translation) is intentional so
        // bare \n from the shell still moves the cursor to column 0.
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(master_fd, &mut termios) == 0 {
                termios.c_lflag &= !(libc::ECHO | libc::ECHOE | libc::ECHOK | libc::ECHONL);
                libc::tcsetattr(master_fd, libc::TCSANOW, &termios);
            }
        };

        {
            let mut state = self.state.lock().unwrap();
            state.cleaned_up = false;
            state.pty_master_fd = Some(master_fd);
            state.cols = cols;
            state.rows = rows;
            state.needs_initial_sigwinch = spawned_hidden;
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
                    state.shell_pid = Some(child.id() as libc::pid_t);
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

/// Decodes percent-encoded URI path components (e.g. `%20` → space).
fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(a), Some(b)) = (h1, h2) {
                if let Ok(byte) = u8::from_str_radix(&format!("{a}{b}"), 16) {
                    out.push(byte as char);
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

/// Maps a 3-bit ANSI color index (0-7) to an RGBA value.
/// `bright` selects the high-intensity variant used by SGR 90-97 / 100-107.
#[inline]
/// Returns the xterm-compatible 16-color ANSI palette as a fallback.
///
/// Indices 0-7 are the normal colors, 8-15 are their bright counterparts.
/// [`TerminalState::apply_theme`] overwrites individual slots with GTK
/// theme values at runtime, so these are only used before the first theme
/// application or for palette entries with no matching GTK variable.
fn default_ansi_palette() -> [gtk::gdk::RGBA; 16] {
    const ENTRIES: [(f32, f32, f32); 16] = [
        (0.0, 0.0, 0.0), // 0  black
        (0.8, 0.0, 0.0), // 1  red
        (0.0, 0.8, 0.0), // 2  green
        (0.8, 0.8, 0.0), // 3  yellow
        (0.0, 0.0, 0.8), // 4  blue
        (0.8, 0.0, 0.8), // 5  magenta
        (0.0, 0.8, 0.8), // 6  cyan
        (0.8, 0.8, 0.8), // 7  white
        (0.4, 0.4, 0.4), // 8  bright black
        (1.0, 0.2, 0.2), // 9  bright red
        (0.2, 1.0, 0.2), // 10 bright green
        (1.0, 1.0, 0.2), // 11 bright yellow
        (0.2, 0.2, 1.0), // 12 bright blue
        (1.0, 0.2, 1.0), // 13 bright magenta
        (0.2, 1.0, 1.0), // 14 bright cyan
        (1.0, 1.0, 1.0), // 15 bright white
    ];
    ENTRIES.map(|(r, g, b)| gtk::gdk::RGBA::new(r, g, b, 1.0))
}

/// Retained for use in [`TerminalState::color_from_256`] (indices 0-15 of the
/// 256-color palette still need a static lookup path).
#[allow(dead_code)]
fn ansi_color(index: u16, bright: bool) -> gtk::gdk::RGBA {
    let palette = default_ansi_palette();
    let idx = if bright {
        8 + (index as usize).min(7)
    } else {
        (index as usize).min(7)
    };
    palette[idx]
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
    // Use pixel height for rendering so content always fills the widget exactly.
    // state.rows may lag by one frame, rendering by pixel avoids over/under draw.
    let visible_rows = ((height as f64) / char_height).ceil() as usize;

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
            // Use accent color for the cursor when the theme has provided one.
            if let Some(acc) = state.accent_color {
                cr.set_source_rgba(
                    acc.red() as f64,
                    acc.green() as f64,
                    acc.blue() as f64,
                    0.75,
                );
            } else {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.3);
            }
            cr.rectangle(x_pos, y_pos, char_width, char_height);
            cr.fill().unwrap();
        }
    }

    draw_scrollbar(cr, state, width, height);
}

/// Draws a 6 px overlay scrollbar on the right edge of the terminal.
///
/// The scrollbar is only rendered when there is scrollback content. The track
/// spans the full widget height, the thumb position reflects the current
/// `scroll_offset` relative to the total content height (scrollback + grid).
/// Both track and thumb are semi-transparent so they sit cleanly over text.
///
/// Geometry contract (must stay in sync with `SCROLLBAR_WIDTH` used by the
/// scrollbar drag gesture in `Terminal::new`):
/// - Track: rightmost `SCROLLBAR_WIDTH` px, full height, rgba(1,1,1,0.06).
/// - Thumb: same x, proportional height, rgba(1,1,1,0.35), minimum 20 px tall.
pub(crate) const SCROLLBAR_WIDTH: f64 = 6.0;

fn draw_scrollbar(cr: &Context, state: &TerminalState, width: i32, height: i32) {
    if state.scrollback.is_empty() {
        return;
    }

    let h = height as f64;
    // Derive visible_rows from current allocated pixel height, not state.rows,
    // so the thumb ratio stays correct after the widget is resized.
    let visible_rows = if state.char_height > 0.0 {
        (h / state.char_height).floor() as usize
    } else {
        state.rows
    };
    let total_rows = state.scrollback.len() + visible_rows;

    // thumb_ratio = fraction of total content that is visible.
    let thumb_ratio = (visible_rows as f64 / total_rows as f64).min(1.0);
    let thumb_h = (h * thumb_ratio).max(20.0);

    // scroll_offset == scrollback.len() means top, 0 means live (bottom).
    let max_offset = state.scrollback.len() as f64;
    let scroll_frac = state.scroll_offset as f64 / max_offset;
    // thumb_y: 0.0 at bottom (live view), h-thumb_h at top (oldest).
    let thumb_y = (h - thumb_h) * scroll_frac;
    // Flip: live view thumb sits at bottom.
    let thumb_y = h - thumb_h - thumb_y;

    let track_x = width as f64 - SCROLLBAR_WIDTH;

    // Track.
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.06);
    cr.rectangle(track_x, 0.0, SCROLLBAR_WIDTH, h);
    cr.fill().unwrap();

    // Thumb.
    let radius = SCROLLBAR_WIDTH / 2.0;
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.35);
    cr.arc(
        track_x + radius,
        thumb_y + radius,
        radius,
        std::f64::consts::PI,
        2.0 * std::f64::consts::PI,
    );
    cr.arc(
        track_x + radius,
        thumb_y + thumb_h - radius,
        radius,
        0.0,
        std::f64::consts::PI,
    );
    cr.close_path();
    cr.fill().unwrap();
}
