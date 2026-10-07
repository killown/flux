use adw::gdk;
use std::path::Path;

use super::png::optimize_png_bytes;

/// Renders the first page of a PDF to a PNG thumbnail and writes it to `cache_path`.
///
/// Scales the page so its longest axis fits within [`constants::CACHED_THUMBNAIL_SIZE`],
/// paints a white background (PDF pages are transparent by default), then serialises
/// the result via `gdk_pixbuf` into the shared XDG thumbnail store so it is
/// session-persistent and reused on subsequent directory loads.
///
/// # Arguments
///
/// * `path`       - Absolute path to the source PDF file.
/// * `cache_path` - Destination `.png` path inside the XDG thumbnail store.
///
/// # Returns
///
/// `Some(texture)` on success, `None` if the document cannot be opened, contains
/// no pages, or the Cairo surface cannot be serialised to PNG.
pub(super) fn pdf_thumbnail(
    path: &Path,
    cache_path: &Path,
    target_size: i32,
) -> Option<gdk::Texture> {
    let doc = poppler::PopplerDocument::new_from_file(path, None).ok()?;
    let page = doc.get_page(0)?;
    let (page_w, page_h) = page.get_size();

    let size = target_size as f64;
    let scale = size / page_w.max(page_h);
    let render_w = (page_w * scale).round() as i32;
    let render_h = (page_h * scale).round() as i32;

    // ARgb32 gives us 4 bytes/pixel (BGRA native order) which matches what
    // `surface.data()` returns and what `Pixbuf::from_bytes` with has_alpha=true expects.
    let mut surface =
        poppler::cairo::ImageSurface::create(poppler::cairo::Format::ARgb32, render_w, render_h)
            .ok()?;
    let cx = poppler::cairo::Context::new(&surface).ok()?;

    // PDF pages are transparent by default, fill white so the thumbnail
    // looks correct on both light and dark file-manager backgrounds.
    cx.set_source_rgb(1.0, 1.0, 1.0);
    cx.paint().ok()?;
    cx.scale(scale, scale);
    page.render(&cx);

    // Drop the context before calling surface.data() - both borrow the surface
    // and Rust enforces that only one mutable borrow exists at a time.
    // Avoids a PNG encode/decode round-trip and sidesteps the `'static` bound
    // on `Pixbuf::from_read`. `ImageSurface::data()` exposes BGRA (cairo native),
    // so has_alpha=true lets gdk_pixbuf handle the channel layout via the stride.
    drop(cx);
    surface.flush();
    let width = surface.width();
    let height = surface.height();
    let stride = surface.stride();
    let data = surface.data().ok()?;

    let pixbuf = gdk_pixbuf::Pixbuf::from_bytes(
        &glib::Bytes::from(&*data),
        gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        width,
        height,
        stride,
    );

    if let Ok(buffer) = pixbuf.save_to_bufferv("png", &[("compression", "9")]) {
        let optimized = optimize_png_bytes(&buffer);
        let _ = std::fs::write(cache_path, optimized);
    }

    Some(gdk::Texture::for_pixbuf(&pixbuf))
}
