use flux::services::terminal::{Cell, TerminalHandler, TerminalState};
use std::sync::{Arc, Mutex};
use vte::Parser;

fn test_state(cols: usize, rows: usize) -> TerminalState {
    TerminalState::new(cols, rows)
}

fn test_handler(state: Arc<Mutex<TerminalState>>) -> TerminalHandler {
    let (tx, _) = tokio::sync::mpsc::unbounded_channel();
    TerminalHandler {
        state,
        draw_sender: tx,
    }
}

fn feed(handler: &mut TerminalHandler, input: &str) {
    let mut parser = Parser::new();
    parser.advance(handler, input.as_bytes());
}

fn feed_bytes(handler: &mut TerminalHandler, bytes: &[u8]) {
    let mut parser = Parser::new();
    parser.advance(handler, bytes);
}

fn assert_row(state: &TerminalState, row: usize, expected: &str) {
    let row_cells = &state.grid[row];
    let mut s = String::with_capacity(row_cells.len());
    for cell in row_cells {
        s.push(cell.ch);
    }
    assert_eq!(s, expected);
}

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

#[test]
fn test_new_state() {
    let state = test_state(80, 24);
    assert_eq!(state.cols, 80);
    assert_eq!(state.rows, 24);
    assert_eq!(state.cursor_x, 0);
    assert_eq!(state.cursor_y, 0);
    assert_eq!(state.grid.len(), 24);
    for row in &state.grid {
        assert_eq!(row.len(), 80);
        for cell in row {
            assert_eq!(cell.ch, ' ');
            assert!(cell.fg.is_none());
            assert!(cell.bg.is_none());
        }
    }
}

#[test]
fn test_resize_shrink() {
    let mut state = test_state(80, 24);
    for col in 0..10 {
        state.grid[0][col].ch = 'a';
    }
    state.resize(10, 5);
    assert_eq!(state.cols, 10);
    assert_eq!(state.rows, 5);
    assert_eq!(state.grid.len(), 5);
    assert_eq!(state.grid[0].len(), 10);
    for col in 0..10 {
        assert_eq!(state.grid[0][col].ch, 'a');
    }
}

#[test]
fn test_resize_expand() {
    let mut state = test_state(10, 5);
    state.grid[0][0].ch = 'x';
    state.resize(80, 24);
    assert_eq!(state.cols, 80);
    assert_eq!(state.rows, 24);
    assert_eq!(state.grid[0][0].ch, 'x');
    assert_eq!(state.grid[0][79].ch, ' ');
    assert_eq!(state.grid[23][0].ch, ' ');
}

#[test]
fn test_scroll_up() {
    let mut state = test_state(5, 3);
    state.grid[0][0].ch = '1';
    state.grid[1][0].ch = '2';
    state.grid[2][0].ch = '3';
    state.scroll_up();
    assert_eq!(state.grid[0][0].ch, '2');
    assert_eq!(state.grid[1][0].ch, '3');
    assert_eq!(state.grid[2][0].ch, ' ');
    assert_eq!(state.scrollback.len(), 1);
    assert_eq!(state.scrollback[0][0].ch, '1');
}

#[test]
fn test_clear() {
    let mut state = test_state(5, 3);
    state.grid[0][0].ch = 'a';
    state.grid[1][1].ch = 'b';
    state.cursor_x = 2;
    state.cursor_y = 1;
    state.clear();
    assert_eq!(state.cursor_x, 0);
    assert_eq!(state.cursor_y, 0);
    for row in &state.grid {
        for cell in row {
            assert_eq!(cell.ch, ' ');
        }
    }
    assert!(state.scrollback.is_empty());
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_scroll_lines() {
    let mut state = test_state(5, 3);
    state.scrollback.push(vec![Cell::blank(); 5]);
    state.scrollback.push(vec![Cell::blank(); 5]);
    state.scrollback.push(vec![Cell::blank(); 5]);
    state.scroll_offset = 0;
    state.scroll_lines(1);
    assert_eq!(state.scroll_offset, 1);
    state.scroll_lines(2);
    assert_eq!(state.scroll_offset, 3);
    state.scroll_lines(-1);
    assert_eq!(state.scroll_offset, 2);
    state.scroll_lines(-5);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_selection_text() {
    let mut state = test_state(10, 3);
    state.grid[0][0].ch = 'a';
    state.grid[0][1].ch = 'b';
    state.grid[0][2].ch = 'c';
    state.selection_start = Some((0, 0));
    state.selection_end = Some((0, 2));
    let text = state.get_selected_text();
    assert_eq!(text, "abc");
}

#[test]
fn test_alt_screen() {
    let mut state = test_state(5, 3);
    state.grid[0][0].ch = 'x';
    state.cursor_x = 2;
    state.cursor_y = 1;
    state.enter_alt_screen();
    assert!(state.grid.iter().all(|row| row.iter().all(|c| c.ch == ' ')));
    assert_eq!(state.cursor_x, 0);
    assert_eq!(state.cursor_y, 0);
    state.exit_alt_screen();
    assert_eq!(state.grid[0][0].ch, 'x');
    assert_eq!(state.cursor_x, 2);
    assert_eq!(state.cursor_y, 1);
}

#[test]
fn test_color_from_256() {
    let color = TerminalState::color_from_256(1);
    assert_eq!(color.red(), 0.8);
    assert_eq!(color.green(), 0.0);
    assert_eq!(color.blue(), 0.0);
    let color = TerminalState::color_from_256(16);
    assert_eq!(color.red(), 0.0);
    assert_eq!(color.green(), 0.0);
    assert_eq!(color.blue(), 0.0);
    let color = TerminalState::color_from_256(232);
    assert_eq!(color.red(), 8.0 / 255.0);
    assert_eq!(color.green(), 8.0 / 255.0);
    assert_eq!(color.blue(), 8.0 / 255.0);
}

#[test]
fn test_print_character() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "Hello");
    {
        let s = state.lock().unwrap();
        assert_row(&s, 0, "Hello     ");
        assert_eq!(s.cursor_x, 5);
        assert_eq!(s.cursor_y, 0);
    }
}

#[test]
fn test_print_wrap() {
    let state = Arc::new(Mutex::new(test_state(5, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "12345");
    {
        let s = state.lock().unwrap();
        assert_row(&s, 0, "12345");
        assert_eq!(s.cursor_x, 4);
        assert_eq!(s.cursor_y, 0);
    }
    feed(&mut handler, "6");
    {
        let s = state.lock().unwrap();
        assert_eq!(s.cursor_x, 1);
        assert_eq!(s.cursor_y, 1);
        assert_row(&s, 1, "6    ");
    }
}

#[test]
fn test_execute_carriage_return() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "Hello\rWorld");
    {
        let s = state.lock().unwrap();
        assert_row(&s, 0, "World     ");
        assert_eq!(s.cursor_x, 5);
    }
}

#[test]
fn test_execute_newline() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "Hello\nWorld");
    {
        let s = state.lock().unwrap();
        assert_row(&s, 0, "Hello     ");
        assert_row(&s, 1, "     World");
        assert_eq!(s.cursor_x, 9);
        assert_eq!(s.cursor_y, 1);
    }
}

#[test]
fn test_execute_tab() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "A\tB");
    {
        let s = state.lock().unwrap();
        assert_row(&s, 0, "A       B ");
        assert_eq!(s.cursor_x, 9);
    }
}

#[test]
fn test_execute_backspace() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "Hello\x08");
    {
        let s = state.lock().unwrap();
        assert_row(&s, 0, "Hello     ");
        assert_eq!(s.cursor_x, 4);
    }
}

#[test]
fn test_csi_cursor_up_down() {
    let state = Arc::new(Mutex::new(test_state(10, 5)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "\x1b[2;2H");
    {
        let s = state.lock().unwrap();
        assert_eq!(s.cursor_x, 1);
        assert_eq!(s.cursor_y, 1);
    }
    feed(&mut handler, "\x1b[A");
    {
        let s = state.lock().unwrap();
        assert_eq!(s.cursor_y, 0);
    }
    feed(&mut handler, "\x1b[2B");
    {
        let s = state.lock().unwrap();
        assert_eq!(s.cursor_y, 2);
    }
    feed(&mut handler, "\x1b[3C");
    {
        let s = state.lock().unwrap();
        assert_eq!(s.cursor_x, 4);
    }
    feed(&mut handler, "\x1b[D");
    {
        let s = state.lock().unwrap();
        assert_eq!(s.cursor_x, 3);
    }
}

#[test]
fn test_csi_erase_display() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "12345\n67890\nABCDE");
    feed(&mut handler, "\x1b[2;1H");
    feed(&mut handler, "\x1b[J");
    {
        let s = state.lock().unwrap();
        assert_row(&s, 1, "          ");
        assert_row(&s, 2, "          ");
        assert_eq!(s.cursor_x, 0);
        assert_eq!(s.cursor_y, 1);
    }
}

#[test]
fn test_csi_erase_line() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "Hello12345");
    feed(&mut handler, "\x1b[6G");
    feed(&mut handler, "\x1b[K");
    {
        let s = state.lock().unwrap();
        assert_row(&s, 0, "Hello     ");
    }
}

#[test]
fn test_csi_insert_lines() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "AAA\nBBB\nCCC");
    feed(&mut handler, "\x1b[1;1H");
    feed(&mut handler, "\x1b[1L");
    {
        let s = state.lock().unwrap();
        assert_row(&s, 0, "          ");
        assert_row(&s, 1, "AAA       ");
        assert_row(&s, 2, "   BBB    ");
        assert_eq!(s.grid.len(), 3);
    }
}

#[test]
fn test_csi_delete_lines() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "AAA\nBBB\nCCC");
    feed(&mut handler, "\x1b[1;1H");
    feed(&mut handler, "\x1b[1M");
    {
        let s = state.lock().unwrap();
        assert_row(&s, 0, "   BBB    ");
        assert_row(&s, 1, "      CCC ");
        assert_row(&s, 2, "          ");
    }
}

#[test]
fn test_csi_sgr_colors() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "\x1b[31mRed\x1b[0mNormal");
    {
        let s = state.lock().unwrap();
        let red = s.ansi_palette[1];
        let cell = &s.grid[0][0];
        assert_eq!(cell.ch, 'R');
        assert_eq!(cell.fg.unwrap(), red);
        let cell_norm = &s.grid[0][6];
        assert_eq!(cell_norm.fg.unwrap(), s.fg_color);
    }
}

#[test]
fn test_csi_sgr_256() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "\x1b[38;5;196mColor");
    {
        let s = state.lock().unwrap();
        let color = TerminalState::color_from_256(196);
        let cell = &s.grid[0][0];
        assert_eq!(cell.fg.unwrap(), color);
    }
}

#[test]
fn test_csi_alt_screen() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "Hello");
    feed(&mut handler, "\x1b[?1049h");
    {
        let s = state.lock().unwrap();
        assert!(s.alt_screen.is_some());
        assert_row(&s, 0, "          ");
    }
    feed(&mut handler, "World");
    feed(&mut handler, "\x1b[?1049l");
    {
        let s = state.lock().unwrap();
        assert!(s.alt_screen.is_none());
        assert_row(&s, 0, "Hello     ");
    }
}

#[test]
fn test_csi_cursor_save_restore() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    feed(&mut handler, "\x1b[2;3H");
    feed(&mut handler, "\x1b[s");
    feed(&mut handler, "\x1b[5;5H");
    feed(&mut handler, "\x1b[u");
    {
        let s = state.lock().unwrap();
        assert_eq!(s.cursor_x, 2);
        assert_eq!(s.cursor_y, 1);
    }
}

#[test]
fn test_osc_7_cwd_change() {
    let state = Arc::new(Mutex::new(test_state(10, 3)));
    let mut handler = test_handler(state.clone());
    let captured = Arc::new(Mutex::new(None));
    {
        let mut s = state.lock().unwrap();
        let cap = captured.clone();
        s.on_cwd_change = Some(Box::new(move |p| {
            *cap.lock().unwrap() = Some(p);
        }));
    }
    feed_bytes(&mut handler, b"\x1b]7;file:///\x1b\\");
    {
        let cap = captured.lock().unwrap();
        assert_eq!(*cap, Some(std::path::PathBuf::from("/")));
    }
}

#[test]
fn test_percent_decode() {
    assert_eq!(percent_decode("hello"), "hello");
    assert_eq!(percent_decode("hello%20world"), "hello world");
    assert_eq!(percent_decode("%41"), "A");
    assert_eq!(percent_decode("%FF"), "\u{ff}");
    assert_eq!(percent_decode("%"), "%");
    assert_eq!(percent_decode("%2"), "%");
}
