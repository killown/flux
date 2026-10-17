//! Icon + embedded metadata for executables (Windows PE, AppImage).
//! Shells out to `file`, `wrestool` and `7z` like the thumbnailers do.

use super::preview::FileInfo;
use crate::i18n::tr;
use std::path::Path;
use std::process::Command;

/// `None` = not a kind we handle. `Some(icon)` = handled, rows were added
/// to `info.extra` and `icon` is the best icon found, if any.
pub fn inspect(path: &Path, ext: &str, info: &mut FileInfo) -> Option<Option<gtk::gdk::Texture>> {
    let is_pe = matches!(ext, "exe" | "dll");
    let is_appimage = ext == "appimage";
    if !is_pe && !is_appimage {
        return None;
    }

    if let Some(summary) = file_summary(path) {
        info.extra.push((tr("Format"), summary));
    }

    if is_pe {
        pe_version_rows(path, info);
        Some(crate::utils::media::generate_thumbnail_sync(path))
    } else {
        appimage_rows(path, info);
        let icon = crate::utils::media::extract_appimage_icon(path)
            .and_then(|p| gtk::gdk::Texture::from_filename(&p).ok());
        Some(icon)
    }
}

fn file_summary(path: &Path) -> Option<String> {
    let out = Command::new("file").arg("-b").arg(path).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !s.is_empty()).then_some(s)
}

/// Reads the VS_VERSION_INFO resource (type 16) and pulls the common string
/// fields out of its UTF-16 payload.
fn pe_version_rows(path: &Path, info: &mut FileInfo) {
    let Ok(out) = Command::new("wrestool")
        .args(["-x", "--raw", "-t16"])
        .arg(path)
        .output()
    else {
        return;
    };
    if !out.status.success() || out.stdout.is_empty() {
        return;
    }

    let units: Vec<u16> = out
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    let strings: Vec<String> = units
        .split(|&u| u == 0)
        .filter(|s| !s.is_empty())
        .map(String::from_utf16_lossy)
        .collect();

    let keys = [
        ("FileDescription", tr("Description")),
        ("ProductName", tr("Product")),
        ("CompanyName", tr("Company")),
        ("FileVersion", tr("Version")),
        ("LegalCopyright", tr("Copyright")),
    ];
    for (key, label) in &keys {
        // Struct headers sit right before each key, so match on the suffix.
        let Some(i) = strings.iter().position(|s| s.ends_with(key)) else {
            continue;
        };
        let Some(val) = strings.get(i + 1).map(|v| v.trim()) else {
            continue;
        };
        // An empty value makes the next string the next key, skip that.
        if val.is_empty() || keys.iter().any(|(k, _)| val.ends_with(k)) {
            continue;
        }
        info.extra.push((label.clone(), val.to_string()));
    }
}

/// Reads the top-level `.desktop` file inside the AppImage with 7z.
fn appimage_rows(path: &Path, info: &mut FileInfo) {
    if !crate::utils::media::seven_zip_available() {
        return;
    }
    let Ok(out) = Command::new("7z")
        .args(["e", "-so", "-y", "-bsp0", "-bse0"])
        .arg(path)
        .arg("*.desktop")
        .output()
    else {
        return;
    };
    if !out.status.success() {
        return;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let wanted = [
        ("Name", tr("Name")),
        ("Comment", tr("Description")),
        ("X-AppImage-Version", tr("Version")),
        ("Categories", tr("Categories")),
    ];
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        if line.starts_with("[Desktop Action") {
            break;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if let Some((_, label)) = wanted.iter().find(|(key, _)| *key == k) {
            if seen.insert(k.to_string()) && !v.trim().is_empty() {
                info.extra.push((label.clone(), v.trim().to_string()));
            }
        }
    }
}
