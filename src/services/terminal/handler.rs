//! VT escape sequence handler that mutates the terminal state.

use super::palette::rgb_color;
use super::state::{Cell, CursorStyle, TerminalState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use vte::Perform;

pub struct TerminalHandler {
    pub state: Arc<Mutex<TerminalState>>,
    /// Set to `true` by the PTY reader thread whenever the grid changes.
    pub needs_redraw: Arc<AtomicBool>,
}

impl Perform for TerminalHandler {
    fn print(&mut self, c: char) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());

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
        self.needs_redraw.store(true, Ordering::Release);
    }

    fn execute(&mut self, byte: u8) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
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
        self.needs_redraw.store(true, Ordering::Release);
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        command: char,
    ) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());

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
            // DECSCUSR - Set cursor style.
            // Ps: 0 = default (reset to Bar), 1/2 = block, 3/4 = underline, 5/6 = bar.
            'q' if intermediates.first().copied() == Some(b' ') => {
                state.cursor_style = match p.first().copied().unwrap_or(0) {
                    1 | 2 => CursorStyle::Block,
                    3 | 4 => CursorStyle::Underline,
                    0 | 5 | 6 => CursorStyle::Bar,
                    _ => CursorStyle::Bar,
                };
            }
            // Ignore unknown sequences per ECMA-48.
            _ => {}
        }
        self.needs_redraw.store(true, Ordering::Release);
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _command: bool) {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());

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
