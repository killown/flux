use adw::gdk;
use std::path::{Path, PathBuf};

use super::png::optimize_png_bytes;

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
