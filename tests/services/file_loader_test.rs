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
    FileLoadContext::with_stats(
        name.to_string(),
        PathBuf::from(name),
        is_dir,
        name.to_lowercase(),
        String::new(),
        size,
        mtime,
        None,
        false,
        None,
    )
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
            SortBy::Size => a.size().cmp(&b.size()),
            SortBy::Date => a.mtime().cmp(&b.mtime()),
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

#[test]
fn test_hwga_bind_icon_lookup_count_matches_disk_entries() {
    if gtk::init().is_err() {
        return;
    }

    flux::utils::helpers::register_resources();

    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    std::fs::create_dir(root.join("docs")).unwrap();
    std::fs::create_dir(root.join("images")).unwrap();
    for i in 0..15 {
        std::fs::write(root.join(format!("file_{}.txt", i)), b"content").unwrap();
    }

    let disk_entries: Vec<std::path::PathBuf> = std::fs::read_dir(root)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    let expected_item_count = disk_entries.len();
    assert_eq!(expected_item_count, 17);

    let display = match gtk::gdk::Display::default() {
        Some(d) => d,
        None => return,
    };
    let icon_theme = gtk::IconTheme::for_display(&display);

    let mut processed_items = Vec::new();
    for path in &disk_entries {
        let is_dir = path.is_dir();
        let icon = flux::utils::get_icon_for_path(path, is_dir);
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        let item = flux::ui::FileItem::builder(name.clone(), path.clone(), icon)
            .display_label(name)
            .is_dir(is_dir)
            .build();
        processed_items.push(item);
    }

    assert_eq!(processed_items.len(), expected_item_count);

    let key = "flux_fm::ui::components::bind_icon_lookup";
    let initial_calls = flux::utils::hwga::CallMonitor::get_stats(key)
        .map(|s| s.count)
        .unwrap_or(0);

    for item in &processed_items {
        let _guard = flux::utils::hwga::CallGuard::new(key);
        let _ = icon_theme.lookup_by_gicon(
            &item.icon,
            48,
            1,
            gtk::TextDirection::None,
            gtk::IconLookupFlags::empty(),
        );
    }

    let final_calls = flux::utils::hwga::CallMonitor::get_stats(key)
        .map(|s| s.count)
        .unwrap_or(0);
    let total_icon_lookups = final_calls - initial_calls;

    assert_eq!(
        total_icon_lookups, expected_item_count,
        "HWGA lookup count ({}) does not match the items found on disk ({})",
        total_icon_lookups, expected_item_count
    );
}
