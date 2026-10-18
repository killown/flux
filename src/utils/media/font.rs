use adw::gdk;
use gtk::pango::prelude::FontMapExt;
use std::fs;
use std::path::Path;

use super::png::store_thumbnail;

/// Private fontconfig config used only for font thumbnails, never the process-wide one.
static PRIVATE_FC: std::sync::Mutex<usize> = std::sync::Mutex::new(0);

mod ffi {
    use std::ffi::c_void;
    use std::os::raw::c_char;

    #[link(name = "fontconfig")]
    extern "C" {
        pub fn FcInitLoadConfigAndFonts() -> *mut c_void;
        pub fn FcConfigAppFontAddFile(config: *mut c_void, file: *const c_char) -> i32;
        pub fn FcConfigAppFontClear(config: *mut c_void);
    }

    #[link(name = "pangoft2-1.0")]
    extern "C" {
        pub fn pango_fc_font_map_set_config(fontmap: *mut c_void, fcconfig: *mut c_void);
    }
}

/// Isolated font map holding one registered font, serialized by a global mutex until dropped.
struct PrivateFontMap {
    // Field order matters: the map must drop before the lock is released.
    map: gtk::pango::FontMap,
    registered: bool,
    _guard: std::sync::MutexGuard<'static, usize>,
}

impl PrivateFontMap {
    fn new(path: &Path) -> Self {
        use glib::prelude::ObjectType;
        use std::os::unix::ffi::OsStrExt;

        let mut guard = PRIVATE_FC.lock().unwrap_or_else(|e| e.into_inner());
        let map = pangocairo::FontMap::new();
        let mut registered = false;

        unsafe {
            if *guard == 0 {
                *guard = ffi::FcInitLoadConfigAndFonts() as usize;
            }
            let config = *guard as *mut std::ffi::c_void;

            if !config.is_null() {
                // Drop the previous thumbnail's font so the config never accumulates fonts.
                ffi::FcConfigAppFontClear(config);
                if let Ok(cpath) = std::ffi::CString::new(path.as_os_str().as_bytes()) {
                    registered = ffi::FcConfigAppFontAddFile(config, cpath.as_ptr()) != 0;
                }
                let raw = map.as_ptr() as *mut std::ffi::c_void;
                ffi::pango_fc_font_map_set_config(raw, config);
            }
        }

        Self {
            map,
            registered,
            _guard: guard,
        }
    }
}

/// Returns `true` if `family` resolves to a real font in `pango_cx`.
///
/// Pango's font map caches the family list per-process, so a font registered
/// with fontconfig *after* the map was created may not be visible until the
/// map is refreshed. This helper lets `font_specimen` decide whether to use
/// the target family or fall back to `"Sans"`.
fn font_family_available(pango_cx: &gtk::pango::Context, family: &str) -> bool {
    use gtk::pango::prelude::{FontFamilyExt, FontMapExt};
    let Some(font_map) = pango_cx.font_map() else {
        return false;
    };
    font_map.list_families().iter().any(|f| f.name() == family)
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
    // Private config under a mutex: never mutate the global fontconfig from worker threads.
    let private = PrivateFontMap::new(path);

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

    let pango_cx = private.map.create_context();
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

    if let Ok(buffer) = pixbuf.save_to_bufferv("png", &[("compression", "1")]) {
        store_thumbnail(&buffer, path, cache_path);
    }

    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

/// Renders a font specimen for the peek-preview panel.
///
/// Unlike [`font_thumbnail`], the result is not written to the thumbnail
/// cache, the panel already has its own selection-scoped lifecycle and this
/// avoids polluting the shared thumbnail store with a preview-only render.
///
/// The layout is three lines sized to fit a square of `target_size`:
///
/// ```text
/// <family name>                    (small, dim, always sans-serif)
/// AaBbCcDdEe 0123456789            (body size, target font)
/// !@#& "(),.,? - ÀÉÎÕÜ ç ñ         (smaller, target font)
/// ```
///
/// Every character in the third line is there because it commonly reveals
/// glyph gaps: `&`, `@`, `?`, quotes, the em-dash, and the accented set most
/// Latin fonts ship with (Portuguese `ã ç õ`, French `À É Î Õ Ü ñ`).
///
/// If the font file could not be registered with fontconfig, or its family
/// name does not resolve through Pango, the body and accent lines fall back
/// to `"Sans"`. The family label line still shows the name read from the
/// file's `name` table, so the user can tell what the file claims to be
/// without seeing a screen of tofu.
pub fn font_specimen(path: &Path, target_size: i32) -> Option<gdk::Texture> {
    let private = PrivateFontMap::new(path);
    let registered = private.registered;

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

    let pango_cx = private.map.create_context();
    pangocairo::functions::update_context(&cx, &pango_cx);

    let family_available = font_family_available(&pango_cx, &family);

    // If the file could not be registered, or Pango has the family name
    // but Cairo cannot open the file, fall back to "Sans" so the specimen
    // still renders. Otherwise Cairo spams "file not found" warnings and
    // draws nothing.
    let registered_ok = registered && font_file_readable(path);
    let body_family: String = if registered_ok && family_available {
        family.clone()
    } else {
        "Sans".to_string()
    };

    let margin = size_f * 0.06;
    let text_w = ((size - (margin * 2.0) as i32) * gtk::pango::SCALE).max(1);

    // Family name: small, dim, fixed sans-serif so it's legible even when
    // the target font's own glyphs are hard to read at label size.
    let label_pt = (size_f * 0.045).max(8.0) as i32;
    let mut label_desc = gtk::pango::FontDescription::new();
    label_desc.set_family("Sans");
    label_desc.set_size(label_pt * gtk::pango::SCALE);

    cx.set_source_rgb(0.4, 0.4, 0.4);
    cx.move_to(margin, margin);
    let label_layout = gtk::pango::Layout::new(&pango_cx);
    label_layout.set_font_description(Some(&label_desc));
    label_layout.set_text(&family);
    label_layout.set_width(text_w);
    label_layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    pangocairo::functions::show_layout(&cx, &label_layout);

    let (_, label_h) = label_layout.pixel_size();

    // Body: alphabet + numerals at the largest readable size.
    let body_pt = (size_f * 0.085).max(10.0) as i32;
    let mut body_desc = gtk::pango::FontDescription::new();
    body_desc.set_family(&body_family);
    body_desc.set_size(body_pt * gtk::pango::SCALE);

    cx.set_source_rgb(0.05, 0.05, 0.05);
    cx.move_to(margin, margin + label_h as f64 + margin * 0.5);
    let body_layout = gtk::pango::Layout::new(&pango_cx);
    body_layout.set_font_description(Some(&body_desc));
    body_layout.set_text("AaBbCcDdEe\n0123456789");
    body_layout.set_width(text_w);
    body_layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    pangocairo::functions::show_layout(&cx, &body_layout);

    let (_, body_h) = body_layout.pixel_size();

    // Punctuation / accents at a smaller body size.
    let accent_pt = (size_f * 0.055).max(9.0) as i32;
    let mut accent_desc = gtk::pango::FontDescription::new();
    accent_desc.set_family(&body_family);
    accent_desc.set_size(accent_pt * gtk::pango::SCALE);

    cx.set_source_rgb(0.15, 0.15, 0.15);
    cx.move_to(margin, margin + label_h as f64 + body_h as f64 + margin);
    let accent_layout = gtk::pango::Layout::new(&pango_cx);
    accent_layout.set_font_description(Some(&accent_desc));

    // Split into two lines so nothing ellipsizes: the whole point is proving
    // glyph coverage, and `ñ ç õ` are exactly the ones a truncated line hides.
    accent_layout.set_text("!@#& \"(),.;? -\nÀÉÎÕÜ ç ñ ã õ");

    accent_layout.set_width(text_w);
    accent_layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    pangocairo::functions::show_layout(&cx, &accent_layout);

    // Capture the accent block's height - needed to position the size ladder
    // below it without overlapping.
    let (_, accent_h) = accent_layout.pixel_size();

    // ── Size ladder ────────────────────────────────────────────────────────
    // Same word at three descending sizes, laid out left to right. Shows how
    // the font's optical weight and letter-spacing behave across sizes,
    // which the fixed-size sample above cannot reveal.
    let sizes = [body_pt, (body_pt * 3) / 4, body_pt / 2];
    let mut x = margin;
    let y = margin + label_h as f64 + body_h as f64 + accent_h as f64 + margin * 1.5;
    for pt in sizes {
        let mut d = gtk::pango::FontDescription::new();
        d.set_family(&body_family);
        d.set_size(pt * gtk::pango::SCALE);
        let l = gtk::pango::Layout::new(&pango_cx);
        l.set_font_description(Some(&d));
        l.set_text("Aa");
        let (w, _) = l.pixel_size();
        cx.move_to(x, y);
        pangocairo::functions::show_layout(&cx, &l);
        x += w as f64 + margin * 0.75;
    }

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

    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

/// Returns `true` if the font file is readable at the path Fontconfig will
/// hand to Cairo. Prevents "file not found" crashes if the temp file was
/// cleaned up concurrently.
fn font_file_readable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}
