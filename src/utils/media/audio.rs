use adw::gdk;
use gdk_pixbuf::prelude::*;
use std::path::Path;

use super::png::optimize_png_bytes;

pub fn audio_thumbnail(path: &Path, cache_path: &Path, target_size: i32) -> Option<gdk::Texture> {
    use lofty::prelude::*;
    use lofty::probe::Probe;

    let tagged_file = Probe::open(path).ok()?.read().ok()?;
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;
    let picture = tag.pictures().first()?;
    let picture_data = picture.data();

    let loader = gdk_pixbuf::PixbufLoader::new();
    loader.write(picture_data).ok()?;
    loader.close().ok()?;
    let pixbuf = loader.pixbuf()?;

    let max_dim = target_size;
    let orig_w = pixbuf.width();
    let orig_h = pixbuf.height();

    let scale = (max_dim as f64 / orig_w.max(orig_h) as f64).min(1.0);
    let new_w = (orig_w as f64 * scale) as i32;
    let new_h = (orig_h as f64 * scale) as i32;

    let scaled = pixbuf.scale_simple(new_w, new_h, gdk_pixbuf::InterpType::Bilinear)?;

    let canvas = gdk_pixbuf::Pixbuf::new(gdk_pixbuf::Colorspace::Rgb, true, 8, max_dim, max_dim)?;

    let x_offset = (max_dim - new_w) / 2;
    let y_offset = (max_dim - new_h) / 2;
    scaled.copy_area(0, 0, new_w, new_h, &canvas, x_offset, y_offset);

    if let Ok(buffer) = canvas.save_to_bufferv("png", &[("compression", "9")]) {
        let optimized = optimize_png_bytes(&buffer);
        let _ = std::fs::write(cache_path, optimized);
    }

    Some(gdk::Texture::for_pixbuf(&canvas))
}
