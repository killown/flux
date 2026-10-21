use adw::gdk;
use adw::prelude::*;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use super::png::store_thumbnail;

/// Returns `true` if `exiftool` is present on `$PATH`.
fn exiftool_available() -> bool {
    crate::hit!("exiftool_available");
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("which")
            .arg("exiftool")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Returns the name of the first available native RAW thumbnailer.
fn find_raw_thumbnailer() -> Option<&'static str> {
    crate::hit!("find_raw_thumbnailer");
    static TOOL: OnceLock<Option<&'static str>> = OnceLock::new();
    *TOOL.get_or_init(|| {
        ["miniaturo", "raw-thumbnailer"].into_iter().find(|tool| {
            Command::new("which")
                .arg(tool)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
    })
}

/// Returns `true` if any RAW thumbnail tool is available.
pub fn raw_thumbnailer_available() -> bool {
    exiftool_available() || find_raw_thumbnailer().is_some()
}

pub(crate) fn raw_thumbnail(
    path: &Path,
    cache_path: &Path,
    cache_dir: &Path,
    target_size: i32,
) -> Option<gdk::Texture> {
    crate::hit!("raw_thumbnail");
    // Primary: exiftool. Correct on every modern camera.
    if exiftool_available() {
        if let Some(bytes) = exiftool_preview(path, target_size) {
            let texture = gdk::Texture::from_bytes(&glib::Bytes::from(&bytes)).ok()?;
            store_thumbnail(&bytes, path, cache_path);
            return Some(texture);
        }
    }

    // Fallback: native thumbnailer. Faster, but broken for some cameras.
    if let Some(tool) = find_raw_thumbnailer() {
        if let Some(tex) = try_raw_thumbnailer(tool, path, cache_dir, target_size) {
            let bytes = std::fs::read(&tex);
            let _ = std::fs::remove_file(&tex);
            let bytes = bytes.ok()?;
            if png_looks_real(&bytes) {
                let texture = gdk::Texture::from_bytes(&glib::Bytes::from(&bytes)).ok()?;
                store_thumbnail(&bytes, path, cache_path);
                return Some(texture);
            }
        }
    }

    None
}

/// Runs the native tool and returns the temp file it wrote, without moving it.
fn try_raw_thumbnailer(
    tool: &str,
    path: &Path,
    cache_dir: &Path,
    target_size: i32,
) -> Option<std::path::PathBuf> {
    crate::hit!("try_raw_thumbnailer");
    let tmp_path = cache_dir.join(format!(
        ".tmp.{}.{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let status = Command::new(tool)
        .arg("-i")
        .arg(path)
        .arg("-o")
        .arg(&tmp_path)
        .arg("-s")
        .arg(target_size.to_string())
        .status()
        .ok()?;

    if !status.success() || !tmp_path.exists() {
        let _ = std::fs::remove_file(&tmp_path);
        return None;
    }

    Some(tmp_path)
}

/// Rejects the black, correctly-sized PNGs that `libopenraw` produces when it
/// mis-parses a modern camera format. A real photographic thumbnail at any
/// usable size is well over 4 KB.
fn png_looks_real(bytes: &[u8]) -> bool {
    crate::hit!("png_looks_real");
    if bytes.len() < 4096 {
        return false;
    }
    let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from(bytes)) else {
        return false;
    };
    texture.width() >= 32 && texture.height() >= 32
}

/// Extracts the embedded JPEG preview via `exiftool` and re-encodes it as a
/// PNG sized to `target_size` on the long edge.
///
/// Tries three tags in order of resolution: `JpgFromRaw` (full-size preview,
/// usually 1-10 MB), `PreviewImage` (screen-size, 50-500 KB), and
/// `ThumbnailImage` (tiny, 5-20 KB). The first tag that returns at least
/// 4 KB of valid image data wins.
fn exiftool_preview(path: &Path, target_size: i32) -> Option<Vec<u8>> {
    crate::hit!("exiftool_preview");
    for tag in ["-JpgFromRaw", "-PreviewImage", "-ThumbnailImage"] {
        let output = Command::new("exiftool")
            .args(["-b", tag])
            .arg(path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;

        if !output.status.success() || output.stdout.len() < 4096 {
            continue;
        }

        let loader = gdk_pixbuf::PixbufLoader::new();
        if loader.write(&output.stdout).is_err() {
            continue;
        }
        if loader.close().is_err() {
            continue;
        }
        let Some(pixbuf) = loader.pixbuf() else {
            continue;
        };

        let src_w = pixbuf.width();
        let src_h = pixbuf.height();
        let scale = (target_size as f64 / src_w.max(src_h) as f64).min(1.0);
        let new_w = (src_w as f64 * scale).round().max(1.0) as i32;
        let new_h = (src_h as f64 * scale).round().max(1.0) as i32;

        let scaled = pixbuf.scale_simple(new_w, new_h, gdk_pixbuf::InterpType::Bilinear)?;
        let bytes = scaled
            .save_to_bufferv("png", &[("compression", "1")])
            .ok()?;
        return Some(bytes);
    }

    None
}
