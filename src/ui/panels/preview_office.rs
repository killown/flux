//! Office-document → PDF conversion helper for the peek-preview panel.
//!
//! Shells out to `soffice --headless --convert-to pdf` and caches the
//! result under `~/.cache/flux/office-previews/<md5>.pdf`, keyed on the
//! source path plus mtime. Reuses the panel's PDF viewer for display.
//!
//! Each invocation gets a private `-env:UserInstallation` profile
//! directory so concurrent conversions cannot deadlock on the shared
//! LibreOffice user profile lock.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Office extensions this module claims. Excludes `.pdf` (handled by the
/// PDF path directly) and `.odf` (rarely the container users expect).
pub const OFFICE_EXTS: &[&str] = &[
    "doc", "docx", "odt", "rtf", "txt2doc", "xls", "xlsx", "ods", "ppt", "pptx", "odp", "csv2doc",
];

/// Returns the first `soffice` / `libreoffice` binary on PATH, or `None`.
/// Probed once per process.
pub fn soffice_binary() -> Option<&'static str> {
    use std::sync::OnceLock;
    static BIN: OnceLock<Option<&'static str>> = OnceLock::new();
    *BIN.get_or_init(|| {
        ["soffice", "libreoffice"].into_iter().find(|&name| {
            Command::new("which")
                .arg(name)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
    })
}

/// Converts `src` to PDF, returns the cached PDF path. `None` on any
/// failure (binary missing, conversion error, unreadable output).
pub fn convert_to_pdf(src: &Path) -> Option<PathBuf> {
    let bin = soffice_binary()?;

    let cache_dir = dirs::cache_dir()?.join("flux").join("office-previews");
    std::fs::create_dir_all(&cache_dir).ok()?;

    let canon = src.canonicalize().ok()?;
    let mtime = std::fs::metadata(&canon).ok()?.modified().ok()?;
    let mtime_secs = mtime
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let key = format!("{}:{}", canon.display(), mtime_secs);
    let hash = format!("{:x}", md5::compute(key.as_bytes()));
    let cached = cache_dir.join(format!("{hash}.pdf"));

    if cached.is_file() {
        return Some(cached);
    }

    let profile = tempfile::Builder::new()
        .prefix("flux-soffice-")
        .tempdir()
        .ok()?;
    let profile_url = format!("file://{}", profile.path().display());

    let out_dir = tempfile::Builder::new()
        .prefix("flux-soffice-out-")
        .tempdir()
        .ok()?;

    let output = Command::new(bin)
        .arg(format!("-env:UserInstallation={profile_url}"))
        .arg("--headless")
        .arg("--norestore")
        .arg("--convert-to")
        .arg("pdf")
        .arg("--outdir")
        .arg(out_dir.path())
        .arg(&canon)
        .output()
        .ok()?;

    if !output.status.success() {
        eprintln!(
            "[flux][preview] soffice failed: status={:?} stdout={:?} stderr={:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return None;
    }

    let stem = canon.file_stem()?.to_string_lossy().to_string();
    let produced = out_dir.path().join(format!("{stem}.pdf"));
    if !produced.is_file() {
        return None;
    }

    let tmp_path = cache_dir.join(format!(".{hash}.pdf.tmp"));
    std::fs::copy(&produced, &tmp_path).ok()?;
    std::fs::rename(&tmp_path, &cached).ok()?;

    Some(cached)
}
