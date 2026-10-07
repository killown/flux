use std::path::Path;

use super::png::optimize_png_bytes;

pub(crate) fn decode_image_thumbnail(
    path: &std::path::Path,
    cache_path: &std::path::Path,
    cache_dir: &std::path::Path,
    target_size: i32,
) -> Option<gtk::gdk::Texture> {
    let pixbuf =
        gdk_pixbuf::Pixbuf::from_file_at_scale(path, target_size, target_size, true).ok()?;

    let tmp_path = cache_dir.join(format!(
        ".tmp.{}.{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let written = pixbuf
        .save_to_bufferv("png", &[("compression", "1")])
        .ok()
        .map(|buf| {
            let optimized = optimize_png_bytes(&buf);
            if std::fs::write(&tmp_path, optimized).is_ok() {
                std::fs::rename(&tmp_path, cache_path).is_ok()
            } else {
                let _ = std::fs::remove_file(&tmp_path);
                false
            }
        })
        .unwrap_or(false);

    if written {
        std::fs::read(cache_path)
            .ok()
            .and_then(|b| gtk::gdk::Texture::from_bytes(&glib::Bytes::from(&b)).ok())
    } else {
        None
    }
}

#[inline]
fn expected_magic(ext: &str) -> Option<&'static [u8]> {
    match ext {
        "png" => Some(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" | "jpe" => Some(b"\xff\xd8\xff"),
        "gif" => Some(b"GIF8"),
        "bmp" => Some(b"BM"),
        "webp" => Some(b"RIFF"),
        "tiff" | "tif" => Some(b"II*\x00"),
        "heic" | "heif" | "avif" => Some(b"\x00\x00\x00"),
        "jxl" => Some(b"\xff\x0a"),
        "ico" | "cur" => Some(b"\x00\x00\x01\x00"),
        "psd" => Some(b"8BPS"),
        _ => None,
    }
}

fn magic_matches(ext: &str, magic: &[u8; 16], len: usize) -> bool {
    let Some(expected) = expected_magic(ext) else {
        return true;
    };
    if len < expected.len() || &magic[..expected.len()] != expected {
        if matches!(ext, "tiff" | "tif") {
            return len >= 4 && &magic[..4] == b"MM\x00*";
        }
        return false;
    }
    if ext == "webp" {
        return len >= 12 && &magic[8..12] == b"WEBP";
    }
    true
}

/// Probes an image file for its pixel dimensions.
///
/// Prefers `imagesize` (pure Rust, header-only, no content-driven allocation).
/// Falls back to `gdk_pixbuf::Pixbuf::file_info` for SVG/XPM/PNM/TGA/ICNS
/// behind a magic-byte check, since glycin can `realloc` gigabytes on a
/// malformed header before the RSS watchdog notices.
///
/// # Arguments
///
/// * `path` - Absolute path to the image file to inspect.
///
/// # Returns
///
/// `Some((width, height))` in pixels on success, `None` otherwise.
#[allow(dead_code)]
pub fn probe_image_dimensions(path: &Path) -> Option<(u32, u32)> {
    let ext_owned = path
        .extension()
        .and_then(|e| e.to_str())?
        .to_ascii_lowercase();
    let ext = ext_owned.as_str();

    let is_img_ext = matches!(
        ext,
        "png"
            | "jpg"
            | "jpeg"
            | "jpe"
            | "webp"
            | "gif"
            | "bmp"
            | "tiff"
            | "tif"
            | "avif"
            | "ico"
            | "cur"
            | "heic"
            | "heif"
            | "jxl"
            | "psd"
            | "svg"
            | "svgz"
            | "icns"
            | "tga"
            | "xpm"
            | "xbm"
            | "pnm"
            | "pbm"
            | "pgm"
            | "ppm"
    );
    if !is_img_ext {
        return None;
    }

    let size = std::fs::metadata(path).ok()?.len();
    if size == 0 || size > 512 * 1024 * 1024 {
        return None;
    }

    if let Ok(dim) = imagesize::size(path) {
        let w = u32::try_from(dim.width).ok()?;
        let h = u32::try_from(dim.height).ok()?;
        if w > 0 && h > 0 && w <= 65_536 && h <= 65_536 {
            return Some((w, h));
        }
        return None;
    }

    use std::io::Read;
    let mut magic = [0u8; 16];
    let mut f = std::fs::File::open(path).ok()?;
    let n = f.read(&mut magic).ok()?;
    if n == 0 || !magic_matches(ext, &magic, n) {
        return None;
    }

    let path_str = path.to_str()?;
    let (_, w, h) = gdk_pixbuf::Pixbuf::file_info(path_str)?;
    let w = u32::try_from(w).ok()?;
    let h = u32::try_from(h).ok()?;

    if w == 0 || h == 0 || w > 65_536 || h > 65_536 {
        return None;
    }

    Some((w, h))
}
