use std::path::{Path, PathBuf};

const MIN_REAL_ICON_BYTES: u64 = 512;

/// Returns `true` if `7z` is present on `$PATH`.
///
/// Probed once per process, the result (positive or negative) is cached so
/// callers on the icon hot path pay a single atomic load after the first call.
pub fn seven_zip_available() -> bool {
    crate::hit!("seven_zip_available");
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        std::process::Command::new("7z")
            .arg("i")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Extracts the icon from an AppImage and caches it as a square PNG.
///
/// Cache lives at `$XDG_CACHE_HOME/flux/appimage-icons/<hash>.png`, keyed on
/// the AppImage's canonical path plus mtime. A cache hit is one `stat`, a
/// miss runs 7z twice at most (once to list, once to extract the chosen
/// entry).
pub fn extract_appimage_icon(appimage: &Path) -> Option<PathBuf> {
    crate::hit!("extract_appimage_icon");
    if !seven_zip_available() {
        return None;
    }

    let cache_dir = dirs::cache_dir()?.join("flux").join("appimage-icons");
    std::fs::create_dir_all(&cache_dir).ok()?;

    let canon = appimage.canonicalize().ok()?;
    let mtime = std::fs::metadata(&canon).ok()?.modified().ok()?;
    let mtime_secs = mtime
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let key = format!("{}:{}", canon.display(), mtime_secs);
    let hash = format!("{:x}", md5::compute(key.as_bytes()));
    let cached = cache_dir.join(format!("{hash}.png"));

    if cached.exists() {
        return Some(cached);
    }

    let entry = find_icon_entry(&canon)?;

    let tmp = tempfile::Builder::new()
        .prefix("flux-appimage-icon-")
        .tempdir()
        .ok()?;

    let status = std::process::Command::new("7z")
        .args(["e", "-y", "-bso0", "-bse0", "-bsp0"])
        .arg(&canon)
        .arg(&entry)
        .arg(format!("-o{}", tmp.path().display()))
        .status()
        .ok()?;

    if !status.success() {
        return None;
    }

    let basename = Path::new(&entry).file_name()?.to_str()?;
    let extracted = tmp.path().join(basename);

    if !extracted.is_file() {
        return None;
    }

    const TARGET: i32 = 512;

    let src = gdk_pixbuf::Pixbuf::from_file_at_scale(&extracted, TARGET, TARGET, true).ok()?;
    let canvas = gdk_pixbuf::Pixbuf::new(gdk_pixbuf::Colorspace::Rgb, true, 8, TARGET, TARGET)?;

    let x = (TARGET - src.width()) / 2;
    let y = (TARGET - src.height()) / 2;
    src.copy_area(0, 0, src.width(), src.height(), &canvas, x, y);

    let bytes = canvas
        .save_to_bufferv("png", &[("compression", "9")])
        .ok()?;
    std::fs::write(&cached, &bytes).ok()?;

    Some(cached)
}

/// Runs `7z l -ba` and picks the best real icon entry.
fn find_icon_entry(appimage: &Path) -> Option<String> {
    crate::hit!("find_icon_entry");
    let output = std::process::Command::new("7z")
        .args(["l", "-ba", "-bso0", "-bse0", "-bsp0"])
        .arg(appimage)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut hicolor_best: Option<(String, u64)> = None;
    let mut top_png_best: Option<(String, u64)> = None;
    let mut top_svg: Option<String> = None;

    for line in stdout.lines() {
        // `7z l -ba` columns: "date time attr size compressed name".
        // The name may contain spaces, so split the first five fields and
        // rejoin the rest.
        let mut it = line.split_whitespace();
        let _date = it.next()?;
        let _time = it.next()?;
        let attr = it.next()?;
        let size: u64 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let _packed = it.next();
        let name: String = it.collect::<Vec<_>>().join(" ");

        if name.is_empty() || attr.starts_with('D') {
            continue;
        }

        if size < MIN_REAL_ICON_BYTES {
            continue;
        }

        let lower = name.to_ascii_lowercase();

        if lower.starts_with("usr/share/icons/hicolor/") && lower.ends_with(".png") {
            if hicolor_best
                .as_ref()
                .map(|(_, s)| size > *s)
                .unwrap_or(true)
            {
                hicolor_best = Some((name.clone(), size));
            }
            continue;
        }

        if !name.contains('/') && lower.ends_with(".png") {
            if top_png_best
                .as_ref()
                .map(|(_, s)| size > *s)
                .unwrap_or(true)
            {
                top_png_best = Some((name, size));
            }
            continue;
        }

        if !name.contains('/') && lower.ends_with(".svg") && top_svg.is_none() {
            top_svg = Some(name);
        }
    }

    hicolor_best
        .map(|(n, _)| n)
        .or_else(|| top_png_best.map(|(n, _)| n))
        .or(top_svg)
}
