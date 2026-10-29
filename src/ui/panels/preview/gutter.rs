//! Line-number gutter for the peek-preview text viewer.

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

fn paint(cr: &Context, tv: &gtk::TextView, _sc: &gtk::ScrolledWindow, w: i32, h: i32) {
    crate::hit!("paint");
    let buf = tv.buffer();
    let line_count = buf.line_count();
    if line_count == 0 {
        return;
    }

    let ctx = tv.pango_context();
    let layout = pango::Layout::new(&ctx);
    if let Some(fd) = ctx.font_description() {
        layout.set_font_description(Some(&fd));
    }

    let digits = line_count.to_string().len() as i32;
    let pad = 8;
    let text_w = digits * 8 + pad * 2;
    let x = (w - text_w).max(0) as f64 + pad as f64;

    cr.set_source_rgba(0.5, 0.5, 0.5, 0.7);

    // line_at_y takes a coordinate relative to the viewport's top edge (0).
    // It returns (TextIter, line_index) of the line at that position.
    let (mut iter, mut n) = tv.line_at_y(0);
    n = n.max(0);

    // If the top of the viewport cuts through a wrapped line, step back to the
    // beginning of that line so we paint its number at the correct spot.
    if !iter.starts_line() && iter.backward_line() {
        n = iter.line();
    }

    // Iterate down through the buffer from the current visible line.
    while n < line_count {
        // Exact pixel position of this line, translated to the viewport.
        let rect = tv.iter_location(&iter);
        let (_, by) = tv.buffer_to_window_coords(gtk::TextWindowType::Text, rect.x(), rect.y());

        // Stop if this line is below the visible gutter area.
        if by > h {
            break;
        }

        // Skip drawing if the line is scrolled above the top.
        if by >= -20 {
            layout.set_text(&format!("{}", n + 1));
            cr.move_to(x, by as f64);
            pangocairo::functions::show_layout(cr, &layout);
        }

        // Advance to the next line in the buffer.
        if !iter.forward_line() {
            break;
        }
        n = iter.line();
    }
}
