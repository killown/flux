//! Non-blocking media metadata extraction.
//!
//! Duration probing uses `ffprobe`. Image dimension probing uses
//! `gdk_pixbuf::Pixbuf::file_info`, which reads only the image header
//! and is already available via the GTK dependency chain.

use std::path::Path;
use std::time::Duration;

/// Probes a media file for its total stream duration without blocking the
/// calling thread.
///
/// Runs `ffprobe` as a subprocess and parses its machine-readable stdout.
/// Returns `None` if the file is not a valid media container, if `ffprobe`
/// is not installed, or if the output cannot be parsed.
///
/// # Arguments
///
/// * `path` - Absolute path to the audio or video file to inspect.
pub fn probe_media_duration(path: &Path) -> Option<Duration> {
    let output = std::process::Command::new("ffprobe")
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

/// Probes an image file for its pixel dimensions by reading only the file
/// header, without decoding the full image into memory.
///
/// Delegates to [`gdk_pixbuf::Pixbuf::file_info`], which is already
/// available through the GTK dependency chain.
///
/// # Arguments
///
/// * `path` - Absolute path to the image file to inspect.
///
/// # Returns
///
/// `Some((width, height))` in pixels on success, `None` otherwise.
pub fn probe_image_dimensions(path: &Path) -> Option<(u32, u32)> {
    let path_str = path.to_str()?;
    let (_, w, h) = gdk_pixbuf::Pixbuf::file_info(path_str)?;
    // file_info returns i32, treat negatives (malformed headers) as unknown
    Some((u32::try_from(w).ok()?, u32::try_from(h).ok()?))
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
        (a, b) = (b, a % b);
    }
    a
}
