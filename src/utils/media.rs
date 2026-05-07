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
    // file_info returns i32; treat negatives (malformed headers) as unknown
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ── aspect_ratio_label ────────────────────────────────────────────────────

    #[test]
    fn test_aspect_ratio_known_16_9() {
        assert_eq!(aspect_ratio_label(1920, 1080), "16:9");
        assert_eq!(aspect_ratio_label(1280, 720), "16:9");
        assert_eq!(aspect_ratio_label(3840, 2160), "16:9");
    }

    #[test]
    fn test_aspect_ratio_known_4_3() {
        assert_eq!(aspect_ratio_label(1024, 768), "4:3");
        assert_eq!(aspect_ratio_label(640, 480), "4:3");
    }

    #[test]
    fn test_aspect_ratio_square() {
        assert_eq!(aspect_ratio_label(512, 512), "1:1");
        assert_eq!(aspect_ratio_label(1, 1), "1:1");
    }

    #[test]
    fn test_aspect_ratio_portrait_9_16() {
        assert_eq!(aspect_ratio_label(1080, 1920), "9:16");
    }

    #[test]
    fn test_aspect_ratio_portrait_2_3() {
        assert_eq!(aspect_ratio_label(2, 3), "2:3");
    }

    #[test]
    fn test_aspect_ratio_non_standard_fallback() {
        // 7:3 is not in the match table, must render as reduced fraction
        assert_eq!(aspect_ratio_label(700, 300), "7:3");
    }

    #[test]
    fn test_aspect_ratio_zero_width() {
        assert_eq!(aspect_ratio_label(0, 1080), "");
    }

    #[test]
    fn test_aspect_ratio_zero_height() {
        assert_eq!(aspect_ratio_label(1920, 0), "");
    }

    #[test]
    fn test_aspect_ratio_both_zero() {
        assert_eq!(aspect_ratio_label(0, 0), "");
    }

    // ── format_duration ───────────────────────────────────────────────────────

    #[test]
    fn test_format_duration_seconds_only() {
        assert_eq!(format_duration(Duration::from_secs(45)), "0:45");
    }

    #[test]
    fn test_format_duration_minutes_and_seconds() {
        assert_eq!(format_duration(Duration::from_secs(125)), "2:05");
    }

    #[test]
    fn test_format_duration_exactly_one_hour() {
        assert_eq!(format_duration(Duration::from_secs(3600)), "1:00:00");
    }

    #[test]
    fn test_format_duration_hours_minutes_seconds() {
        assert_eq!(format_duration(Duration::from_secs(3723)), "1:02:03");
    }

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0:00");
    }

    #[test]
    fn test_format_duration_sub_second_truncates() {
        // Sub-second precision must be truncated, not rounded
        assert_eq!(format_duration(Duration::from_millis(59_999)), "0:59");
    }

    // ── gcd (via aspect_ratio_label) ──────────────────────────────────────────

    #[test]
    fn test_gcd_coprime_inputs_produce_unreduced_ratio() {
        // 7 and 9 are coprime, reduced ratio must equal the original
        assert_eq!(aspect_ratio_label(7, 9), "7:9");
    }

    #[test]
    fn test_gcd_large_common_factor() {
        // 1000×1000 = 1:1 after GCD(1000,1000)=1000
        assert_eq!(aspect_ratio_label(1000, 1000), "1:1");
    }
}
