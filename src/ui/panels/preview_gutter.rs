//! Line-number gutter for the peek-preview text viewer.
//!
//! A `gtk::DrawingArea` parked to the left of the `TextView`, painting only
//! the visible rows. Same "visible-range painter" shape as the terminal's
//! `draw_scrollbar` in `src/services/terminal/draw.rs`.

use gtk::cairo::Context;
use gtk::prelude::*;

pub struct LineNumberGutter {
    pub area: gtk::DrawingArea,
}

impl LineNumberGutter {
    pub fn new(text_view: &gtk::TextView, scroller: &gtk::ScrolledWindow) -> Self {
        let area = gtk::DrawingArea::builder()
            .width_request(44)
            .vexpand(true)
            .css_classes(["flux-preview-gutter"])
            .build();

        let tv = text_view.clone();
        let sc = scroller.clone();

        area.set_draw_func(move |_, cr, w, h| {
            paint(cr, &tv, &sc, w, h);
        });

        // Repaint when the buffer changes.
        text_view.buffer().connect_changed({
            let a = area.clone();
            move |_| a.queue_draw()
        });

        // Repaint when the viewport scrolls.
        {
            let a = area.clone();
            scroller
                .vadjustment()
                .connect_value_changed(move |_| a.queue_draw());
        }
        {
            let a = area.clone();
            scroller
                .vadjustment()
                .connect_page_size_notify(move |_| a.queue_draw());
        }
        {
            let a = area.clone();
            scroller
                .vadjustment()
                .connect_upper_notify(move |_| a.queue_draw());
        }

        Self { area }
    }
}

fn paint(cr: &Context, tv: &gtk::TextView, sc: &gtk::ScrolledWindow, w: i32, h: i32) {
    let buf = tv.buffer();
    let line_count = buf.line_count();
    if line_count == 0 {
        return;
    }

    // Derive the line height from a pango layout of the current font.
    let ctx = tv.pango_context();
    let layout = pango::Layout::new(&ctx);
    if let Some(fd) = ctx.font_description() {
        layout.set_font_description(Some(&fd));
    }
    layout.set_text("0");
    let lh = layout.pixel_extents().1.height() as f64;
    if lh <= 0.0 {
        return;
    }

    let vadj = sc.vadjustment();
    let val = vadj.value();
    let top_margin = tv.top_margin() as f64;

    let first_visible = ((val - top_margin) / lh).floor().max(0.0) as i32;
    let last_visible = (((val + h as f64) - top_margin) / lh).ceil() as i32;
    let last_visible = last_visible.min(line_count);
    if last_visible <= first_visible {
        return;
    }

    let digits = line_count.to_string().len() as i32;
    let pad = 8;
    let text_w = digits * 8 + pad * 2;
    let x = (w - text_w).max(0) as f64 + pad as f64;

    cr.set_source_rgba(0.5, 0.5, 0.5, 0.7);
    for n in first_visible..last_visible {
        let Some(mut iter) = buf.iter_at_line(n) else {
            continue;
        };
        if iter.line() != n {
            continue;
        }
        let y = top_margin + (n as f64) * lh - val;
        if y + lh < 0.0 || y > h as f64 {
            continue;
        }
        layout.set_text(&format!("{}", n + 1));
        cr.move_to(x, y);
        pangocairo::functions::show_layout(cr, &layout);
        let _ = &mut iter;
    }
}
