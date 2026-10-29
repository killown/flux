//! PDF preview: page navigation, resize-driven re-rendering and the Poppler
//! rasteriser, which runs on a blocking worker and never on the GTK thread.

use super::content::PreviewContent;
use super::handle::PreviewPanelHandle;
use super::state::Mode;
use crate::i18n::tr;
use adw::prelude::*;
use std::path::Path;
use std::time::Duration;

const MAX_PDF_PX: i32 = 4096;
const MAX_PDF_PX_H: i32 = 16384;
pub(super) const PDF_GUTTER: i32 = 24;

#[cfg(target_endian = "little")]
const CAIRO_ARGB32: gtk::gdk::MemoryFormat = gtk::gdk::MemoryFormat::B8g8r8a8Premultiplied;
#[cfg(target_endian = "big")]
const CAIRO_ARGB32: gtk::gdk::MemoryFormat = gtk::gdk::MemoryFormat::A8r8g8b8Premultiplied;

impl PreviewPanelHandle {
    // ── PDF ─────────────────────────────────────────────────────────────────

    pub(super) fn sync_nav(&self) {
        let st = &self.state;
        st.suppress.set(true);
        self.page_spin.set_value((st.page.get() + 1) as f64);
        st.suppress.set(false);
        self.prev_btn.set_sensitive(st.page.get() > 0);
        self.next_btn
            .set_sensitive(st.page.get() + 1 < st.pages.get());
    }

    pub(super) fn go_to_page(&self, idx: usize) {
        crate::hit!("go_to_page");
        let st = &self.state;
        let pages = st.pages.get();
        if pages == 0 {
            return;
        }
        let idx = idx.min(pages - 1);
        if idx == st.page.get() {
            self.sync_nav();
            return;
        }
        st.page.set(idx);
        self.sync_nav();
        self.pdf_scroller.vadjustment().set_value(0.0);
        self.render_pdf();
    }

    pub(super) fn schedule_resize(&self) {
        crate::hit!("schedule_resize");
        let st = &self.state;
        let gen = st.resize_gen.get() + 1;
        st.resize_gen.set(gen);
        let h = self.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(160), move || {
            if h.state.resize_gen.get() != gen {
                return;
            }
            match h.state.mode.get() {
                Mode::Image => {
                    let view = (h.image_scroller.width(), h.image_scroller.height());
                    if view != h.state.last_image_view.get() {
                        h.relayout_image();
                    }
                }
                Mode::Pdf => {
                    let want = h.desired_pdf_logical_w();
                    if want > 0 && (want - h.state.rendered_w.get()).abs() > 2 {
                        h.render_pdf();
                    }
                }
                _ => {}
            }
        });
    }

    fn desired_pdf_logical_w(&self) -> i32 {
        let vw = self.stack.width();
        if vw <= 5 {
            return 0;
        }
        // Render at 50% of the available width so there is breathing room
        // for the navigation arrows, but leave it to the zoom multiplier to
        // scale up if the user wants it larger.
        ((vw as f64) * 0.5 * self.state.zoom.get())
            .round()
            .max(32.0) as i32
    }

    pub(super) fn render_pdf(&self) {
        let token = self.state.token.get() + 1;
        self.state.token.set(token);
        self.render_pdf_inner(token, 0);
    }

    fn render_pdf_inner(&self, token: u64, attempt: u32) {
        crate::hit!("render_pdf_inner");
        if self.state.token.get() != token {
            return;
        }
        let Some(path) = self.state.pdf_path.borrow().clone() else {
            return;
        };

        let logical_w = self.desired_pdf_logical_w();
        if logical_w <= 0 {
            // The stack has no allocation yet, retry briefly while the
            // revealer animates in.
            if attempt < 40 {
                let h = self.clone();
                gtk::glib::timeout_add_local_once(Duration::from_millis(50), move || {
                    h.render_pdf_inner(token, attempt + 1);
                });
            }
            return;
        }

        let sf = self.stack.scale_factor().max(1);
        let target_px = (logical_w * sf).min(MAX_PDF_PX);
        let page = self.state.page.get();

        // Poppler rendering is synchronous and CPU-bound, keep it off the
        // GTK main thread.
        let job = gtk::gio::spawn_blocking(move || render_pdf_raw(&path, page, target_px));
        let this = self.clone();
        gtk::glib::spawn_future_local(async move {
            let raw = job.await.ok().flatten();
            if this.state.token.get() != token {
                return;
            }
            let Some(raw) = raw else {
                this.apply(&PreviewContent::Error(tr("Could not render this page.")));
                return;
            };
            let bytes = glib::Bytes::from_owned(raw.bytes);
            let tex = gtk::gdk::MemoryTexture::new(
                raw.w,
                raw.h,
                CAIRO_ARGB32,
                &bytes,
                raw.stride as usize,
            );
            this.pdf_picture.set_paintable(Some(&tex));
            this.pdf_picture.set_size_request(
                (raw.w as f64 / sf as f64).round() as i32,
                (raw.h as f64 / sf as f64).round() as i32,
            );
            this.state.rendered_w.set(logical_w);
            this.stack.set_visible_child_name("pdf");
        });
    }
}

// ─── PDF rasteriser (worker thread) ──────────────────────────────────────────

struct RawPage {
    bytes: Vec<u8>,
    w: i32,
    h: i32,
    stride: i32,
}

fn render_pdf_raw(path: &Path, page_index: usize, target_w: i32) -> Option<RawPage> {
    crate::hit!("render_pdf_raw");
    let doc = poppler::PopplerDocument::new_from_file(path, None).ok()?;
    let page = doc.get_page(page_index)?;
    let (pw, ph) = page.get_size();
    if pw <= 0.0 || ph <= 0.0 {
        return None;
    }

    let mut scale = target_w as f64 / pw;
    if ph * scale > MAX_PDF_PX_H as f64 {
        scale = MAX_PDF_PX_H as f64 / ph;
    }
    let rw = (pw * scale).round().max(1.0) as i32;
    let rh = (ph * scale).round().max(1.0) as i32;

    let mut surface =
        poppler::cairo::ImageSurface::create(poppler::cairo::Format::ARgb32, rw, rh).ok()?;
    {
        let cx = poppler::cairo::Context::new(&surface).ok()?;
        cx.set_source_rgb(1.0, 1.0, 1.0);
        cx.paint().ok()?;
        cx.scale(rw as f64 / pw, rh as f64 / ph);
        page.render(&cx);
    }
    surface.flush();

    let stride = surface.stride();
    let bytes = surface.data().ok()?.to_vec();
    Some(RawPage {
        bytes,
        w: rw,
        h: rh,
        stride,
    })
}
