use std::path::Path;
use std::time::Duration;

use super::png::optimize_png_bytes;

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
