//! Non-blocking media duration extraction via `ffprobe`.
//!
//! Uses the same `ffprobe` binary that ships alongside `ffmpeg`, which is
//! already required for video thumbnail generation in `utils::core`.

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
///
/// # Returns
///
/// `Some(Duration)` on success, `None` on any failure.
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

    // ffprobe returns "N/A" for container formats with no duration header
    if stdout == "N/A" || stdout.is_empty() {
        return None;
    }

    let secs: f64 = stdout.parse().ok()?;

    // Reject non-finite values (e.g. live streams can return infinity)
    if !secs.is_finite() || secs < 0.0 {
        return None;
    }

    Some(Duration::from_secs_f64(secs))
}

/// Formats a [`Duration`] into a human-readable `H:MM:SS` or `M:SS` string.
///
/// # Arguments
///
/// * `d` - The duration to format.
///
/// # Returns
///
/// A `String` in `H:MM:SS` format when the duration is one hour or longer,
/// or `M:SS` format otherwise.
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
