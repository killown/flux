//! Installs font files into the user's font directory.

use crate::i18n::{ntr, tr};
use std::io;
use std::path::{Path, PathBuf};

const FONT_EXTS: &[&str] = &[
    "ttf", "otf", "ttc", "otc", "woff", "woff2", "pfb", "pfa", "pcf", "bdf", "otb", "dfont",
];

/// Returns whether `path` has a font file extension.
pub fn is_font_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| FONT_EXTS.iter().any(|f| e.eq_ignore_ascii_case(f)))
}

/// Returns the per-user font directory (`~/.local/share/fonts`).
pub fn user_font_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("fonts"))
}

/// Outcome counters for one install request.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct InstallReport {
    pub installed: usize,
    pub already: usize,
    pub failed: usize,
}

impl InstallReport {
    /// Builds the toast text for this report.
    pub fn message(&self) -> String {
        if self.installed > 0 {
            let n = self.installed as u64;
            let mut msg =
                ntr("Installed 1 font", "Installed {} fonts", n).replace("{}", &n.to_string());
            if self.failed > 0 {
                msg.push_str(&format!(" ({} {})", self.failed, tr("failed")));
            }
            msg
        } else if self.failed == 0 && self.already > 0 {
            tr("Already installed")
        } else {
            tr("Could not install the font")
        }
    }
}

/// Resolves the readable source file and the destination file name, extracting archive entries first.
fn resolve_source(path: &Path) -> Option<(PathBuf, String)> {
    let raw = path.to_string_lossy();
    if raw.starts_with(crate::infra::archive::ARCHIVE_URI) {
        let (archive, inner) = crate::infra::archive::parse_archive_uri(&raw)?;
        let name = Path::new(&inner)
            .file_name()?
            .to_string_lossy()
            .into_owned();
        let real = crate::infra::archive::extract_archive_entry_for_playback(&archive, &inner)?;
        Some((real, name))
    } else {
        let name = path.file_name()?.to_string_lossy().into_owned();
        Some((path.to_path_buf(), name))
    }
}

/// Returns whether both files have identical bytes.
fn same_content(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) else {
        return false;
    };
    ma.len() == mb.len() && matches!((std::fs::read(a), std::fs::read(b)), (Ok(x), Ok(y)) if x == y)
}

/// Picks `name`, or `stem-N.ext` when `name` is already taken in `dir`.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    crate::hit!("unique_path");
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let p = Path::new(name);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (1..10_000)
        .map(|i| dir.join(format!("{stem}-{i}{ext}")))
        .find(|c| !c.exists())
        .unwrap_or(first)
}

/// Copies one font into `dir`, `Ok(true)` when installed, `Ok(false)` when an identical file exists.
fn install_one(path: &Path, dir: &Path) -> io::Result<bool> {
    crate::hit!("install_one");
    let (src, name) = resolve_source(path)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "font source unavailable"))?;

    let direct = dir.join(&name);
    if direct.exists() && same_content(&src, &direct) {
        return Ok(false);
    }

    let dest = unique_path(dir, &name);
    std::fs::copy(&src, &dest)?;
    Ok(true)
}

/// Refreshes the fontconfig cache for `dir`, ignoring a missing `fc-cache`.
fn refresh_font_cache(dir: &Path) {
    crate::hit!("refresh_font_cache");
    let _ = std::process::Command::new("fc-cache")
        .arg(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Installs `paths` into the user font directory and refreshes the font cache, blocking.
pub fn install_fonts(paths: &[PathBuf]) -> InstallReport {
    crate::hit!("install_fonts");
    let mut report = InstallReport::default();

    let dir = match user_font_dir() {
        Some(d) if std::fs::create_dir_all(&d).is_ok() => d,
        _ => {
            report.failed = paths.len();
            return report;
        }
    };

    for path in paths.iter().filter(|p| is_font_path(p)) {
        match install_one(path, &dir) {
            Ok(true) => report.installed += 1,
            Ok(false) => report.already += 1,
            Err(_) => report.failed += 1,
        }
    }

    if report.installed > 0 {
        refresh_font_cache(&dir);
    }
    report
}

/// Cheap check (name and size only) for whether a local font is already in the user font dir.
pub fn is_installed(path: &Path) -> bool {
    crate::hit!("is_installed");
    if path
        .to_string_lossy()
        .starts_with(crate::infra::archive::ARCHIVE_URI)
    {
        return false;
    }
    let (Some(dir), Some(name)) = (user_font_dir(), path.file_name()) else {
        return false;
    };
    match (std::fs::metadata(path), std::fs::metadata(dir.join(name))) {
        (Ok(a), Ok(b)) => a.len() == b.len(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_font_extensions() {
        assert!(is_font_path(Path::new("/x/Foo.TTF")));
        assert!(is_font_path(Path::new("a.woff2")));
        assert!(!is_font_path(Path::new("a.txt")));
        assert!(!is_font_path(Path::new("ttf")));
    }

    #[test]
    fn unique_path_adds_suffix() {
        let dir = std::env::temp_dir().join(format!("flux-fonts-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.ttf"), b"x").unwrap();
        assert_eq!(unique_path(&dir, "a.ttf"), dir.join("a-1.ttf"));
        assert_eq!(unique_path(&dir, "b.ttf"), dir.join("b.ttf"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
