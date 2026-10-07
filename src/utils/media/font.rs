use adw::gdk;
use gtk::pango::prelude::FontMapExt;
use std::fs;
use std::path::Path;

use super::png::optimize_png_bytes;

/// Registers a font file with the process-local fontconfig instance so Pango
/// can resolve it without system installation.
///
/// Calls `FcConfigAppFontAddFile` from libfontconfig (a transitive system
/// dependency of GTK/Pango - always present on the target platform).  The
/// registration is process-local and cleaned up on exit, no global state is
/// modified.
fn register_font_with_fontconfig(path: &Path) {
    // SAFETY: libfontconfig is guaranteed present (GTK transitive dep).
    // FcConfigAppFontAddFile(NULL, path) adds `path` to the default config's
    // application font list.  NULL config pointer → current default config.
    // The path string is valid for the duration of the call.
    #[link(name = "fontconfig")]
    extern "C" {
        fn FcConfigAppFontAddFile(
            config: *mut std::ffi::c_void,
            file: *const std::os::raw::c_char,
        ) -> i32;
    }

    if let Ok(cpath) = std::ffi::CString::new(path.to_string_lossy().as_bytes()) {
        unsafe {
            FcConfigAppFontAddFile(std::ptr::null_mut(), cpath.as_ptr());
        }
    }
}

/// Renders a font preview thumbnail using PangoCairo and writes it to `cache_path`.
///
/// Loads the font file directly into a temporary [`pango::FontMap`] override via
/// `fc-cache`-free [`fontconfig`] font loading, then lays out a two-line sample:
/// the font family name on top and the pangram `"AaBbCc 123"` below in the target
/// font at a size scaled to fill [`constants::CACHED_THUMBNAIL_SIZE`].
///
/// The background is white with dark text so thumbnails are legible on both light
/// and dark file-manager themes.
///
/// # Arguments
///
/// * `path`       - Absolute path to the source font file.
/// * `cache_path` - Destination `.png` path inside the XDG thumbnail store.
///
/// # Returns
///
/// `Some(texture)` on success, `None` if the font cannot be loaded or the Cairo
/// surface cannot be read back.
/// Reads the font family name from a TrueType/OpenType `name` table (nameID 1).
///
/// Parses just enough of the binary `name` table to extract the English family
/// name without pulling in a full font parsing library.  Returns `None` if the
/// file cannot be read or the table is malformed, the caller falls back to the
/// filename stem in that case.
fn read_font_family(path: &Path) -> Option<String> {
    let data = fs::read(path).ok()?;

    // Offset table: 12 bytes header + 16 bytes per table record.
    // We scan the table directory for the 'name' tag (0x6E616D65).
    if data.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([*data.get(4)?, *data.get(5)?]) as usize;
    let dir_start = 12usize;

    let mut name_offset = None;
    for i in 0..num_tables {
        let base = dir_start.checked_add(i.checked_mul(16)?)?;
        if base.checked_add(16)? > data.len() {
            break;
        }
        let tag = data.get(base..base + 4)?;
        if tag == b"name" {
            let offset = u32::from_be_bytes([
                *data.get(base + 8)?,
                *data.get(base + 9)?,
                *data.get(base + 10)?,
                *data.get(base + 11)?,
            ]) as usize;
            name_offset = Some(offset);
            break;
        }
    }

    let name_base = name_offset?;
    if name_base.checked_add(6)? > data.len() {
        return None;
    }

    let count = u16::from_be_bytes([*data.get(name_base + 2)?, *data.get(name_base + 3)?]) as usize;
    let string_offset =
        u16::from_be_bytes([*data.get(name_base + 4)?, *data.get(name_base + 5)?]) as usize;
    let storage = name_base.checked_add(string_offset)?;

    // Scan name records (12 bytes each) for nameID=1 (Family), platformID=3 (Windows), encodingID=1 (Unicode BMP).
    // Fall back to platformID=1 (Mac) if no Windows record found.
    let mut family_win: Option<String> = None;
    let mut family_mac: Option<String> = None;

    for i in 0..count {
        let rec = name_base.checked_add(6)?.checked_add(i.checked_mul(12)?)?;
        if rec.checked_add(12)? > data.len() {
            break;
        }
        let platform_id = u16::from_be_bytes([*data.get(rec)?, *data.get(rec + 1)?]);
        let encoding_id = u16::from_be_bytes([*data.get(rec + 2)?, *data.get(rec + 3)?]);
        let name_id = u16::from_be_bytes([*data.get(rec + 6)?, *data.get(rec + 7)?]);
        let length = u16::from_be_bytes([*data.get(rec + 8)?, *data.get(rec + 9)?]) as usize;
        let offset = u16::from_be_bytes([*data.get(rec + 10)?, *data.get(rec + 11)?]) as usize;

        if name_id != 1 {
            continue;
        }

        let start = storage.checked_add(offset)?;
        let end = start.checked_add(length)?;
        if end > data.len() {
            continue;
        }
        let raw = data.get(start..end)?;

        if platform_id == 3 && encoding_id == 1 && family_win.is_none() {
            // UTF-16 BE
            #[allow(clippy::chunks_exact_to_as_chunks)]
            let chars: Vec<u16> = raw
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect();
            if let Ok(s) = String::from_utf16(&chars) {
                family_win = Some(s);
            }
        } else if platform_id == 1 && family_mac.is_none() {
            // Mac Roman - ASCII-compatible for Latin family names
            family_mac = Some(String::from_utf8_lossy(raw).into_owned());
        }
    }

    family_win.or(family_mac)
}

pub fn font_thumbnail(path: &Path, cache_path: &Path, target_size: i32) -> Option<gdk::Texture> {
    // Register the font file with fontconfig so Pango can load it by family
    // name without requiring system installation.
    register_font_with_fontconfig(path);

    // Prefer the actual internal family name from the binary name table,
    // fall back to the filename stem if parsing fails.
    let family = read_font_family(path)
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Sans".to_string());

    let size = target_size;
    let size_f = size as f64;

    let mut surface =
        pangocairo::cairo::ImageSurface::create(pangocairo::cairo::Format::ARgb32, size, size)
            .ok()?;
    let cx = pangocairo::cairo::Context::new(&surface).ok()?;

    cx.set_source_rgb(1.0, 1.0, 1.0);
    cx.paint().ok()?;

    let font_map = pangocairo::FontMap::new();
    let pango_cx = font_map.create_context();
    pangocairo::functions::update_context(&cx, &pango_cx);

    // Sample lines: family name hint at top, pangram below.
    let sample_body = "AaBbCc 123\nThe quick fox";
    let body_pt = (size_f * 0.22) as i32;
    let mut body_desc = gtk::pango::FontDescription::new();
    body_desc.set_family(&family);
    body_desc.set_size(body_pt * gtk::pango::SCALE);

    // Small label at the top: family name in a neutral sans so it's always legible.
    let label_pt = (size_f * 0.09) as i32;
    let mut label_desc = gtk::pango::FontDescription::new();
    label_desc.set_family("Sans");
    label_desc.set_size(label_pt * gtk::pango::SCALE);

    let margin = size_f * 0.06;

    // Draw the family name label.
    cx.set_source_rgb(0.4, 0.4, 0.4);
    cx.move_to(margin, margin);
    let label_layout = gtk::pango::Layout::new(&pango_cx);
    label_layout.set_font_description(Some(&label_desc));
    label_layout.set_text(&family);
    label_layout.set_width((size - (margin * 2.0) as i32) * gtk::pango::SCALE);
    label_layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    pangocairo::functions::show_layout(&cx, &label_layout);

    // Draw the pangram sample in the target font.
    cx.set_source_rgb(0.05, 0.05, 0.05);
    let (_, label_h) = label_layout.pixel_size();
    cx.move_to(margin, margin + label_h as f64 + margin * 0.25);
    let body_layout = gtk::pango::Layout::new(&pango_cx);
    body_layout.set_font_description(Some(&body_desc));
    body_layout.set_text(sample_body);
    body_layout.set_width((size - (margin * 2.0) as i32) * gtk::pango::SCALE);
    body_layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    pangocairo::functions::show_layout(&cx, &body_layout);

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
