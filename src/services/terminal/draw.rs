//! Cairo/Pango rendering of the terminal grid and scrollbar.

use super::state::{CursorStyle, TerminalState};
use gtk::cairo::Context;
use gtk::prelude::*;
use gtk::DrawingArea;

pub(super) fn draw_terminal(
    area: &DrawingArea,
    cr: &Context,
    state: &TerminalState,
    width: i32,
    height: i32,
) {
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

    if state.scroll_offset == 0 && state.cursor_visible && state.cursor_blink_visible {
        let cursor_x = state.cursor_x;
        let cursor_y = state.cursor_y;
        if cursor_y < state.rows && cursor_x < state.cols {
            let x_pos = cursor_x as f64 * char_width;
            let y_pos = cursor_y as f64 * char_height;

            if let Some(acc) = state.accent_color {
                cr.set_source_rgba(
                    acc.red() as f64,
                    acc.green() as f64,
                    acc.blue() as f64,
                    0.85,
                );
            } else {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.85);
            }

            match state.cursor_style {
                CursorStyle::Block => {
                    cr.rectangle(x_pos, y_pos, char_width, char_height);
                }
                CursorStyle::Underline => {
                    let bar_h = (char_height * 0.1).max(2.0);
                    cr.rectangle(x_pos, y_pos + char_height - bar_h, char_width, bar_h);
                }
                CursorStyle::Bar => {
                    let bar_w = (char_width * 0.12).max(2.0);
                    cr.rectangle(x_pos, y_pos, bar_w, char_height);
                }
            }
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
