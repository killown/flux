//! Runtime availability checks for optional external binaries.

/// Logs a warning for each optional external binary not found in `$PATH`.
pub fn check_optional_deps() {
    for bin in ["ffmpeg", "ffprobe", "magick"] {
        if std::process::Command::new("which")
            .arg(bin)
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
        {
            eprintln!("[flux] optional dependency '{bin}' not found in PATH");
        }
    }
}
