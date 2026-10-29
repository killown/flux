//! Zoom handling shared by the viewers, and image layout.

use super::handle::PreviewPanelHandle;
use super::state::Mode;
use adw::prelude::*;

const ZOOM_MIN: f64 = 0.25;
const ZOOM_MAX: f64 = 5.0;
pub(super) const ZOOM_STEP: f64 = 1.25;

impl PreviewPanelHandle {
    // ── Zoom ────────────────────────────────────────────────────────────────

    pub(super) fn zoom_by(&self, factor: f64) {
        self.set_zoom(self.state.zoom.get() * factor);
    }

    pub(super) fn set_zoom(&self, z: f64) {
        let z = z.clamp(ZOOM_MIN, ZOOM_MAX);
        let z = if (z - 1.0).abs() < 0.03 { 1.0 } else { z };
        self.state.zoom.set(z);
        self.sync_zoom_label();
        match self.state.mode.get() {
            Mode::Pdf => self.render_pdf(),
            Mode::Image => self.relayout_image(),
            Mode::Text | Mode::Markdown => self.text_tag.set_scale(z),
            Mode::Table | Mode::None => {}
        }
    }

    pub(super) fn sync_zoom_label(&self) {
        self.zoom_btn.set_label(&format!(
            "{}%",
            (self.state.zoom.get() * 100.0).round() as i32
        ));
    }

    // ── Image ───────────────────────────────────────────────────────────────

    pub(super) fn relayout_image(&self) {
        crate::hit!("relayout_image");
        let (tw, th) = self.state.tex_px.get();
        let (vw, vh) = (self.image_scroller.width(), self.image_scroller.height());
        if tw <= 0 || th <= 0 || vw <= 0 || vh <= 0 {
            return;
        }
        self.state.last_image_view.set((vw, vh));
        let sf = self.stack.scale_factor().max(1) as f64;
        let (lw, lh) = (tw as f64 / sf, th as f64 / sf);
        let fit = (vw as f64 / lw).min(vh as f64 / lh).min(1.0);
        let z = self.state.zoom.get();
        self.image.set_size_request(
            (lw * fit * z).round().max(1.0) as i32,
            (lh * fit * z).round().max(1.0) as i32,
        );
    }
}
