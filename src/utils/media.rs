//! Non-blocking media metadata extraction.
//!
//! Duration probing uses `ffprobe`. Image dimension probing prefers `imagesize`
//! (pure Rust, header-only) and falls back to `gdk_pixbuf::Pixbuf::file_info`
//! behind a magic-byte check, since glycin can OOM on malformed headers.

use std::path::Path;
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
