use flux::model::FileLoadContext;
use flux::model::SortBy;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
fn is_visual_media_by_ext(path: &Path) -> (bool, bool) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some(
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "heic" | "heif" | "bmp" | "tiff"
            | "tif" | "jxl" | "svg" | "pdf" | "ttf" | "otf" | "woff" | "woff2" | "ttc",
        ) => (true, false),
        Some(
            "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "wmv" | "m4v" | "mpg" | "mpeg" | "ts"
            | "ogv",
        ) => (false, true),
        _ => (false, false),
    }
}

#[allow(dead_code)]
fn xbel_attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{}=\"", name);
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

#[allow(dead_code)]
fn mock_ctx(name: &str, is_dir: bool, size: u64, mtime: i64) -> FileLoadContext {
    FileLoadContext {
        display_name: name.to_string(),
        sort_name: name.to_lowercase(),
        sort_ext: String::new(),
        target_path: PathBuf::from(name),
        size,
        mtime,
        is_dir,
        thumbnail_path: None,
        is_foreign_owner: false,
        expand_labels: false,
        custom_icon: None,
    }
}

#[allow(dead_code)]
fn sort_items(items: &mut [FileLoadContext], by: SortBy, folders_first: bool, ascending: bool) {
    items.par_sort_unstable_by(|a, b| {
        if a.is_dir != b.is_dir {
            return if folders_first {
                b.is_dir.cmp(&a.is_dir)
            } else {
                a.is_dir.cmp(&b.is_dir)
            };
        }
        let primary = match by {
            SortBy::Name => a.sort_name.cmp(&b.sort_name),
            SortBy::Size => a.size.cmp(&b.size),
            SortBy::Date => a.mtime.cmp(&b.mtime),
            SortBy::Type => {
                let ext_a = Path::new(&a.display_name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let ext_b = Path::new(&b.display_name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                ext_a.cmp(&ext_b)
            }
        };
        let tie = if primary == std::cmp::Ordering::Equal {
            a.sort_name.cmp(&b.sort_name)
        } else {
            primary
        };
        if ascending {
            tie
        } else {
            tie.reverse()
        }
    });
}
