//! Non-blocking media metadata extraction.
//!
//! Duration probing uses `ffprobe`. Image dimension probing prefers `imagesize`
//! (pure Rust, header-only) and falls back to `gdk_pixbuf::Pixbuf::file_info`
//! behind a magic-byte check, since glycin can OOM on malformed headers.

use adw::gdk;
use gdk_pixbuf::prelude::PixbufLoaderExt;
use gio::prelude::FileExt;
use gtk::pango::prelude::FontMapExt;
use oxipng::{Options, StripChunks};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Probes a media file for its total stream duration asynchronously without blocking the
/// thread pool worker.
///
/// Runs `ffprobe` as a Tokio child process and parses its stdout.
/// Returns `None` if the file is not a valid media container, if `ffprobe`
/// is not installed, or if the output cannot be parsed.
#[allow(dead_code)]
pub async fn probe_media_duration(path: &Path) -> Option<Duration> {
    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = std::str::from_utf8(&output.stdout).ok()?.trim();

    if stdout == "N/A" || stdout.is_empty() {
        return None;
    }

    let secs: f64 = stdout.parse().ok()?;

    if !secs.is_finite() || secs < 0.0 {
        return None;
    }

    Some(Duration::from_secs_f64(secs))
}

/// Optimizes raw PNG bytes in-place using oxipng.
pub fn optimize_png_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut opts = Options::from_preset(2);
    opts.strip = StripChunks::All;

    oxipng::optimize_from_memory(bytes, &opts).unwrap_or_else(|_| bytes.to_vec())
}

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

/// Retrieves a thumbnail [`gdk::Texture`] for a visual media file, generating
/// and caching it on first access.
///
/// Cache entries follow the [FreeDesktop Thumbnail Managing Standard], stored under
/// `$XDG_CACHE_HOME/thumbnails/<size-tier>/` as `MD5("file://<path>").hex + ".png"`.
/// This makes cache hits session-persistent: a thumbnail written in a previous
/// session is reused without regeneration as long as the source file's `mtime` and
/// size have not changed.
///
/// Stale entries (source modified after thumbnail was written, or zero-byte files)
/// are evicted and regenerated atomically.
///
/// [FreeDesktop Thumbnail Managing Standard]: https://specifications.freedesktop.org/thumbnail-spec/
///
/// # Arguments
///
/// * `path` - Absolute path to an image or video file.
///
/// # Returns
///
/// `Some(texture)` on success, `None` if the file is not visual media, if any
/// required external tool (`ffmpeg`) is unavailable, or if I/O fails.
pub async fn get_or_create_thumbnail(path: &std::path::Path) -> Option<gtk::gdk::Texture> {
    crate::hit!("get_or_create_thumbnail");
    let path = path.to_path_buf();

    // WARNING: Never generate thumbnails for files inside the thumbnail cache itself.
    // Prevents an infinite recursive generation loop when browsing ~/.cache/thumbnails/
    if let Some(cache_root) = dirs::cache_dir() {
        if path.starts_with(cache_root.join("thumbnails")) {
            return None;
        }
    }

    tokio::task::spawn_blocking(move || generate_thumbnail_sync(&path))
        .await
        .ok()
        .flatten()
}

/// Resolves the FreeDesktop-compliant thumbnail cache path for a given source file.
///
/// [Thumbnail Managing Standard]: https://specifications.freedesktop.org/thumbnail-spec/
///
/// # Arguments
///
/// * `path` - Absolute path to the source media file.
///
/// # Returns
///
/// `(cache_dir, cache_path)` where `cache_dir` is the resolved size-tier directory
/// and `cache_path` is the full `.png` destination. Returns `None` if
/// `dirs::cache_dir()` is unavailable or `path` contains non-UTF-8 bytes.
fn thumbnail_cache_path(path: &Path, target_size: i32) -> Option<(PathBuf, PathBuf, String)> {
    let thumb_folder = match target_size {
        s if s > 512 => "xx-large",
        s if s > 256 => "x-large",
        s if s > 128 => "large",
        _ => "normal",
    };

    let cache_dir = dirs::cache_dir()?.join("thumbnails").join(thumb_folder);

    let uri = gio::File::for_path(path).uri();
    let hash = format!("{:x}", md5::compute(uri.as_bytes()));
    let cache_path = cache_dir.join(format!("{hash}.png"));

    Some((cache_dir, cache_path, hash))
}

#[allow(dead_code)]
fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

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
fn pdf_thumbnail(path: &Path, cache_path: &Path, target_size: i32) -> Option<gdk::Texture> {
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

/// Returns `true` if `path` is a font file by extension (case-insensitive).
#[allow(dead_code)]
fn is_font(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "ttf" | "otf" | "woff" | "woff2" | "ttc"
        )
    })
}

/// Returns whether a cached thumbnail PNG is still valid for the given source file.
///
/// Compares the source's `mtime` against the thumbnail's own `mtime`. A thumbnail
/// is considered stale when the source was modified after the thumbnail was written,
/// or when the on-disk entry is zero bytes (partial write / crash residue).
///
/// # Arguments
///
/// * `cache_path`   - Path to the candidate `.png` thumbnail.
/// * `source_meta`  - [`fs::Metadata`] of the original source file.
fn thumbnail_is_valid(cache_path: &Path, source_meta: &fs::Metadata) -> bool {
    let cache_meta = match fs::metadata(cache_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let cache_mtime = cache_meta.modified().ok();
    let source_mtime = source_meta.modified().ok();

    // Source newer than cache → stale.
    if let (Some(ct), Some(st)) = (cache_mtime, source_mtime) {
        if st > ct {
            return false;
        }
    }

    // Guard against zero-byte truncation or partial writes.
    cache_meta.len() > 0
}

pub(crate) fn generate_thumbnail_sync(path: &std::path::Path) -> Option<gtk::gdk::Texture> {
    let path_str = path.to_string_lossy();
    if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
        return crate::services::archive::get_archive_thumbnail(path);
    }

    let (show_thumbnails, target_size, thumb_types, _ffmpeg_threads, _auto_rotate, _seek_secs) =
        crate::utils::config::get_thumb_config();

    if !show_thumbnails {
        return None;
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    let is_pdf_file = ext == "pdf";
    let is_font_file =
        !is_pdf_file && matches!(ext.as_str(), "ttf" | "otf" | "woff" | "woff2" | "ttc");
    let is_exe_file = !is_pdf_file && !is_font_file && ext == "exe";
    let is_audio = !is_pdf_file
        && !is_font_file
        && !is_exe_file
        && matches!(
            ext.as_str(),
            "mp3" | "flac" | "m4a" | "ogg" | "wav" | "aac" | "wma" | "opus"
        );

    let (is_img, is_vid) = if !is_pdf_file && !is_font_file && !is_exe_file && !is_audio {
        match ext.as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "heic" | "heif" | "bmp" | "tiff"
            | "tif" | "jxl" | "svg" | "ico" => (true, false),
            "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "wmv" | "m4v" | "mpg" | "mpeg"
            | "ts" | "ogv" => (false, true),
            _ => {
                let filename = path.file_name().unwrap_or_default().to_string_lossy();
                let (ct, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
                (ct.starts_with("image/"), ct.starts_with("video/"))
            }
        }
    } else {
        (false, false)
    };

    let is_supported = (is_pdf_file && thumb_types.pdfs)
        || (is_font_file && thumb_types.fonts)
        || (is_img && thumb_types.images)
        || (is_vid && thumb_types.videos)
        || (is_exe_file && thumb_types.executables)
        || (is_audio && thumb_types.audio);

    if !is_supported {
        return None;
    }

    let (cache_dir, cache_path, hash) = thumbnail_cache_path(path, target_size)?;

    let source_meta = std::fs::metadata(path).ok();

    let existing_thumbnail: Option<PathBuf> = if cache_path.exists() {
        Some(cache_path.clone())
    } else if let Some(base_cache) = dirs::cache_dir().map(|c| c.join("thumbnails")) {
        ["xx-large", "x-large", "large", "normal"]
            .iter()
            .map(|tier| base_cache.join(tier).join(format!("{hash}.png")))
            .find(|p| p.exists())
    } else {
        None
    };

    if let Some(existing) = existing_thumbnail {
        if let Ok(meta) = std::fs::metadata(&existing) {
            if meta.len() > 1024 * 1024 {
                let _ = std::fs::remove_file(&existing);
            } else {
                let is_valid = source_meta
                    .as_ref()
                    .map(|m| thumbnail_is_valid(&existing, m))
                    .unwrap_or(true);

                if is_valid {
                    if let Ok(bytes) = std::fs::read(&existing) {
                        if let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from(&bytes)) {
                            return Some(texture);
                        }
                    }
                }
                let _ = std::fs::remove_file(&existing);
            }
        }
    }

    let _ = std::fs::create_dir_all(&cache_dir);

    if is_img {
        return crate::utils::media::decode_image_thumbnail(
            path,
            &cache_path,
            &cache_dir,
            target_size,
        );
    }

    if is_vid {
        return crate::utils::media::extract_video_frame(
            path,
            &cache_path,
            &cache_dir,
            target_size,
        );
    }

    if is_pdf_file {
        return pdf_thumbnail(path, &cache_path, target_size);
    }

    if is_font_file {
        return crate::utils::media::font_thumbnail(path, &cache_path, target_size);
    }

    if is_audio {
        return crate::utils::media::audio_thumbnail(path, &cache_path, target_size);
    }

    if is_exe_file {
        return crate::utils::media::extract_exe_icon(path, &cache_path, target_size);
    }

    None
}

/// Returns `true` when the theme's icon for `content_type` is a hit on a
/// container MIME that many unrelated extensions share (`.conf`, `.dat`,
/// `.cfg` all collapse to `application/xml` or `text/plain`), and the
/// extension itself is not that container's subtype. In that case the theme
/// icon is not evidence the theme can draw this extension, so generation
/// should run instead.
#[inline]
pub fn container_mime_masks_extension(ext: &str, content_type: &str) -> bool {
    if ext.is_empty() {
        return false;
    }

    let is_container = matches!(
        content_type,
        "application/xml"
            | "text/xml"
            | "application/json"
            | "application/octet-stream"
            | "application/x-zerosize"
            | "application/x-empty"
    );
    if !is_container {
        return false;
    }

    // If the extension literally IS the subtype (`.xml` → `xml`), the MIME is
    // specific and the theme icon is authoritative.
    let ext_is_subtype = content_type
        .split('/')
        .nth(1)
        .map(|s| s.trim_start_matches("x-").eq_ignore_ascii_case(ext))
        .unwrap_or(false);

    !ext_is_subtype
}

pub fn icon_gen_params() -> (bool, String, String, String, f64) {
    let (_, _, auto_gen, accent, body, font, font_size) = crate::utils::config::get_icon_config();
    (auto_gen, accent, body, font, font_size)
}

pub fn get_mime_type(path: &Path) -> String {
    let path_str = path.to_string_lossy();

    if path_str.starts_with(crate::services::archive::ARCHIVE_URI) {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(mime) = guess_mime_from_extension(ext) {
                return mime;
            }
        }
        return "application/octet-stream".to_string();
    }

    if crate::services::network::is_network_uri(path) {
        if path_str.ends_with('/') {
            return "inode/directory".to_string();
        }

        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        if filename.is_empty() {
            return "inode/directory".to_string();
        }

        let (content_type, _) = gio::content_type_guess(Some(filename.as_ref()), None);
        let ct = content_type.to_string();

        if ct == "inode/directory" {
            return guess_mime_from_extension(&filename)
                .unwrap_or_else(|| "application/octet-stream".to_string());
        }
        return ct;
    }

    if path.is_dir() {
        return "inode/directory".to_string();
    }

    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let mut sniff_buffer = [0u8; 4096];
    let data_slice = if let Ok(mut file) = fs::File::open(path) {
        if let Ok(count) = file.read(&mut sniff_buffer) {
            Some(&sniff_buffer[..count])
        } else {
            None
        }
    } else {
        None
    };

    let (content_type, _) = gio::content_type_guess(Some(filename.as_ref()), data_slice);
    let ct = content_type.to_string();

    if ct == "inode/directory" && path.is_file() {
        return guess_mime_from_extension(&filename)
            .unwrap_or_else(|| "application/octet-stream".to_string());
    }

    ct
}

pub fn guess_mime_from_extension(filename: &str) -> Option<String> {
    let ext = std::path::Path::new(filename).extension()?.to_str()?;
    crate::utils::extension_template::lookup_system_extension_mime(ext)
}

pub fn is_visual_media(path: &Path) -> (bool, bool) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some(
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "heic" | "heif" | "bmp" | "tiff"
            | "tif" | "jxl" | "svg",
        ) => (true, false),
        Some(
            "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "wmv" | "m4v" | "mpg" | "mpeg" | "ts"
            | "ogv",
        ) => (false, true),
        // only hit GIO for unknown extensions
        _ => {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            let (ct, _) = adw::gio::content_type_guess(Some(filename.as_ref()), None);
            (ct.starts_with("image/"), ct.starts_with("video/"))
        }
    }
}

pub fn is_audio_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "mp3" | "flac" | "m4a" | "ogg" | "wav"
        )
    })
}

/// Extracts the highest resolution icon embedded inside a Windows PE executable.
pub fn extract_exe_icon(
    exe_path: &Path,
    cache_path: &Path,
    target_size: i32,
) -> Option<gdk::Texture> {
    let temp_dir = tempfile::Builder::new()
        .prefix("flux-exe-icon-")
        .tempdir()
        .ok()?;

    let ico_path = temp_dir.path().join("icon.ico");

    // Extract default group icon (type 14) directly to .ico
    let status = std::process::Command::new("wrestool")
        .args(["-x", "-t14", "-o"])
        .arg(&ico_path)
        .arg(exe_path)
        .status()
        .ok()?;

    if !status.success() || !ico_path.exists() {
        return None;
    }

    let png_dir = temp_dir.path().join("png");
    std::fs::create_dir_all(&png_dir).ok()?;

    let icotool_status = std::process::Command::new("icotool")
        .args(["-x", "-o"])
        .arg(&png_dir)
        .arg(&ico_path)
        .status()
        .ok()?;

    if !icotool_status.success() {
        return None;
    }

    let mut best_img: Option<PathBuf> = None;
    let mut best_dim: u32 = 0;

    if let Ok(entries) = std::fs::read_dir(&png_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("png") {
                continue;
            }
            let dim = p
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| {
                    s.rsplit('_')
                        .find_map(|part| part.split('x').next()?.parse::<u32>().ok())
                })
                .unwrap_or(0);

            if dim > best_dim {
                best_dim = dim;
                best_img = Some(p);
            }
        }
    }

    let source_png = best_img?;

    let pixbuf =
        gdk_pixbuf::Pixbuf::from_file_at_scale(&source_png, target_size, target_size, true).ok()?;

    if let Ok(buffer) = pixbuf.save_to_bufferv("png", &[("compression", "9")]) {
        let optimized = optimize_png_bytes(&buffer);
        let _ = std::fs::write(cache_path, optimized);
    }

    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

pub(crate) fn extract_video_frame(
    path: &std::path::Path,
    cache_path: &std::path::Path,
    cache_dir: &std::path::Path,
    target_size: i32,
) -> Option<gtk::gdk::Texture> {
    let config = crate::utils::load_config();
    let seek_secs = config.ui.ffmpeg_seek_seconds;
    let threads_str = config.ui.ffmpeg_threads.to_string();
    let auto_rotate = config.ui.ffmpeg_auto_rotate;

    let tmp_path = cache_dir.join(format!(
        ".tmp.{}.{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let run_ffmpeg = |seek: &str| -> bool {
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.arg("-y").arg("-loglevel").arg("panic");
        if !auto_rotate {
            cmd.arg("-noautorotate");
        }
        cmd.arg("-ss")
            .arg(seek)
            .arg("-i")
            .arg(path)
            .arg("-an")
            .arg("-threads")
            .arg(&threads_str)
            .arg("-vframes")
            .arg("1")
            .arg("-vf")
            .arg(format!(
                "scale={target_size}:-1:force_original_aspect_ratio=decrease"
            ))
            .arg(&tmp_path);

        cmd.status().map(|s| s.success()).unwrap_or(false)
    };

    let seek_str = format!("{seek_secs:.3}");
    let seek_is_nonzero = seek_secs > 0.001;

    let mut success = if seek_is_nonzero {
        run_ffmpeg(&seek_str)
    } else {
        false
    };

    if !success || !tmp_path.exists() {
        success = run_ffmpeg("0.000");
    }

    if success && tmp_path.exists() {
        if let Ok(raw_png) = std::fs::read(&tmp_path) {
            let optimized = optimize_png_bytes(&raw_png);
            let _ = std::fs::write(&tmp_path, optimized);
        }
        let _ = std::fs::rename(&tmp_path, cache_path);
        if let Ok(bytes) = std::fs::read(cache_path) {
            return gtk::gdk::Texture::from_bytes(&glib::Bytes::from(&bytes)).ok();
        }
    } else {
        let _ = std::fs::remove_file(&tmp_path);
    }

    None
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

/// Returns a canonical aspect ratio label for a given resolution.
///
/// Reduces `width × height` by their GCD, then matches common display ratios.
/// Falls back to the reduced fraction string for non-standard ratios.
///
/// # Arguments
///
/// * `w` - Image width in pixels.
/// * `h` - Image height in pixels.
#[allow(dead_code)]
pub fn aspect_ratio_label(w: u32, h: u32) -> String {
    if w == 0 || h == 0 {
        return String::new();
    }

    let g = gcd(w, h);
    let rw = w / g;
    let rh = h / g;

    match (rw, rh) {
        (16, 9) => "16:9".into(),
        (4, 3) => "4:3".into(),
        (21, 9) => "21:9".into(),
        (1, 1) => "1:1".into(),
        (3, 2) => "3:2".into(),
        (5, 4) => "5:4".into(),
        (16, 10) => "16:10".into(),
        (9, 16) => "9:16".into(),
        (2, 3) => "2:3".into(),
        _ => format!("{}:{}", rw, rh),
    }
}

/// Formats a [`Duration`] into a human-readable `H:MM:SS` or `M:SS` string.
///
/// # Arguments
///
/// * `d` - The duration to format.
#[allow(dead_code)]
pub fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;

    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let rem = a % b;
        a = b;
        b = rem;
    }
    a
}
