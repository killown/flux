//! Status and label formatting helpers.

use crate::i18n::tr;
use relm4::prelude::*;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Computes the right-side status string containing active filters and free volume space.
pub fn format_right_status(
    current_path: &Path,
    extension_filter: Option<&[String]>,
    filter_mode: crate::model::FilterMode,
    mime_filter: Option<(&str, usize)>,
) -> String {
    let mut parts = Vec::new();

    // Add filter mode indicator
    match filter_mode {
        crate::model::FilterMode::OnlyFolders => parts.push(tr("[Folders Only]")),
        crate::model::FilterMode::OnlyFiles => parts.push(tr("[Files Only]")),
        crate::model::FilterMode::All => {}
    }

    if let Some((mime, count)) = mime_filter {
        parts.push(format!(
            "[{}: {} ({} {})]",
            tr("mime"),
            mime,
            count,
            tr("items")
        ));
    }

    if let Some(patterns) = extension_filter {
        if !patterns.is_empty() {
            parts.push(format!("[filter: {}]", patterns.join(", ")));
        }
    }

    if current_path.is_absolute() && current_path.exists() {
        if let Ok(c_path) = CString::new(current_path.as_os_str().as_bytes()) {
            let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
            if unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) } == 0 {
                let stat = unsafe { stat.assume_init() };
                let free_bytes = (stat.f_bsize) * (stat.f_bavail);
                parts.push(format!(
                    "{} {}",
                    gtk::glib::format_size(free_bytes),
                    tr("free")
                ));
            }
        }
    }

    parts.join(" · ")
}

/// Checks if a paste operation is recursive (pasting a folder into itself or a subfolder).
#[allow(dead_code)]
pub fn is_recursive_paste(src: &Path, dest_dir: &Path) -> bool {
    dest_dir.starts_with(src)
}

/// Returns the label to display for a file in the grid or list.
///
/// If `name`'s extension is in `hidden_extensions` (case-insensitive, optional leading dot),
/// or if `hidden_extensions` contains `*`, returns the stem only. Otherwise returns `name`.
pub fn format_display_label(name: &str, is_dir: bool, hidden_extensions: &[String]) -> String {
    if is_dir || hidden_extensions.is_empty() {
        return name.to_string();
    }

    let hide_all = hidden_extensions.iter().any(|h| {
        let trimmed = h.trim();
        trimmed == "*" || trimmed == ".*"
    });

    let mut patterns: Vec<String> = hidden_extensions
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty() && s != "*" && s != ".*")
        .collect();

    patterns.sort_by_key(|a| std::cmp::Reverse(a.len()));

    let name_lc = name.to_ascii_lowercase();

    let glob_matchers: Vec<globset::GlobMatcher> = patterns
        .iter()
        .filter(|c| c.contains('*') || c.contains('?') || c.contains('['))
        .filter_map(|clean| {
            let pat = if clean.starts_with('*') {
                clean.clone()
            } else if clean.starts_with('.') {
                format!("*{}", clean)
            } else {
                format!("*.{}", clean)
            };
            globset::GlobBuilder::new(&pat)
                .case_insensitive(true)
                .literal_separator(false)
                .build()
                .ok()
                .map(|g| g.compile_matcher())
        })
        .collect();

    for matcher in &glob_matchers {
        if matcher.is_match(&name_lc) {
            for (byte_idx, _) in name.match_indices('.') {
                if byte_idx > 0 && byte_idx < name.len() {
                    let ext_slice = &name_lc[byte_idx..];
                    let test_str = format!("x{}", ext_slice);
                    if matcher.is_match(&test_str) {
                        return name[..byte_idx].to_string();
                    }
                }
            }
        }
    }

    for clean in &patterns {
        if clean.contains('*') || clean.contains('?') || clean.contains('[') {
            continue;
        }

        let suffix = if clean.starts_with('.') {
            clean.clone()
        } else {
            format!(".{}", clean)
        };

        if suffix.is_empty() || suffix == "." {
            continue;
        }

        if name_lc.ends_with(&suffix) {
            let remaining_len = name.len().saturating_sub(suffix.len());
            if remaining_len > 0 && name.is_char_boundary(remaining_len) {
                return name[..remaining_len].to_string();
            }
        }
    }

    if hide_all {
        return std::path::Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name)
            .to_string();
    }

    name.to_string()
}
