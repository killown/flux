//! Terminal grid state: cells, cursor and screen buffers.

use super::palette::default_ansi_palette;
use std::os::unix::io::RawFd;

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

#[derive(Clone, Debug, PartialEq)]
pub enum CursorStyle {
    Block,     // DECSCUSR 1/2
    Underline, // DECSCUSR 3/4
    Bar,       // DECSCUSR 5/6
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
pub(super) fn pty_is_raw(fd: libc::c_int) -> bool {
    let mut termios = unsafe { std::mem::zeroed::<libc::termios>() };
    let ret = unsafe { libc::tcgetattr(fd, &mut termios) };
    if ret != 0 {
        return false;
    }
    termios.c_lflag & libc::ICANON == 0
}

pub struct TerminalState {
    pub cursor_style: CursorStyle,
    pub cursor_blink_visible: bool,
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
            cursor_style: CursorStyle::Bar,
            cursor_blink_visible: true,
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

    pub(super) fn reset_attrs(&mut self) {
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
