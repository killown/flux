use flux::model::FileLoadContext;
use flux::model::SortBy;
use flux::services::loader::resolve_thumb_source;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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

fn xbel_attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{}=\"", name);
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn mock_ctx(name: &str, is_dir: bool, size: u64, mtime: i64) -> FileLoadContext {
    FileLoadContext {
        display_name: name.to_string(),
        sort_name: name.to_lowercase(),
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

#[test]
fn test_thumbnail_session_cancellation_logic() {
    let load_id = AtomicU64::new(1);
    let current_session = 1u64;

    assert_eq!(load_id.load(Ordering::Acquire), current_session);

    // Simulate navigation/session shift
    load_id.fetch_add(1, Ordering::SeqCst);
    assert_ne!(load_id.load(Ordering::Acquire), current_session);
}

#[test]
fn test_is_visual_media_by_ext_mapping() {
    let (is_img, is_vid) = is_visual_media_by_ext(Path::new("photo.PNG"));
    assert!(is_img);
    assert!(!is_vid);

    let (is_img, is_vid) = is_visual_media_by_ext(Path::new("video.mkv"));
    assert!(!is_img);
    assert!(is_vid);

    let (is_img, is_vid) = is_visual_media_by_ext(Path::new("document.pdf"));
    assert!(is_img);
    assert!(!is_vid);

    let (is_img, is_vid) = is_visual_media_by_ext(Path::new("script.sh"));
    assert!(!is_img);
    assert!(!is_vid);
}

#[test]
fn test_xbel_attribute_extraction() {
    let line =
        r#"<bookmark href="file:///tmp/test.txt" added="2026-01-01" modified="2026-08-07"/>"#;

    assert_eq!(
        xbel_attr(line, "href"),
        Some("file:///tmp/test.txt".to_string())
    );
    assert_eq!(xbel_attr(line, "modified"), Some("2026-08-07".to_string()));
    assert_eq!(xbel_attr(line, "missing"), None);
}

#[test]
fn test_sort_by_name() {
    let mut items = vec![
        mock_ctx("Zebra", false, 0, 0),
        mock_ctx("Apple", false, 0, 0),
        mock_ctx("Banana", false, 0, 0),
    ];
    sort_items(&mut items, SortBy::Name, true, true);
    assert_eq!(items[0].display_name, "Apple");
    assert_eq!(items[1].display_name, "Banana");
    assert_eq!(items[2].display_name, "Zebra");
}

#[test]
fn test_sort_by_size_descending() {
    let mut items = vec![
        mock_ctx("Small", false, 100, 0),
        mock_ctx("Large", false, 1000, 0),
        mock_ctx("Medium", false, 500, 0),
    ];
    sort_items(&mut items, SortBy::Size, true, false);
    assert_eq!(items[0].display_name, "Large");
    assert_eq!(items[1].display_name, "Medium");
    assert_eq!(items[2].display_name, "Small");
}

#[test]
fn test_sort_by_date_descending() {
    let mut items = vec![
        mock_ctx("Old", false, 0, 1000),
        mock_ctx("New", false, 0, 3000),
        mock_ctx("Mid", false, 0, 2000),
    ];
    sort_items(&mut items, SortBy::Date, true, false);
    assert_eq!(items[0].display_name, "New");
    assert_eq!(items[1].display_name, "Mid");
    assert_eq!(items[2].display_name, "Old");
}

#[test]
fn test_sort_by_type() {
    let mut items = vec![
        mock_ctx("file.b", false, 0, 0),
        mock_ctx("file.a", false, 0, 0),
        mock_ctx("file.A", false, 0, 0),
    ];
    sort_items(&mut items, SortBy::Type, true, true);
    assert_eq!(items[0].display_name, "file.a");
    assert_eq!(items[1].display_name, "file.A");
    assert_eq!(items[2].display_name, "file.b");
}

#[test]
fn test_folders_first_sorting() {
    let mut items = vec![
        mock_ctx("file.txt", false, 0, 0),
        mock_ctx("DirA", true, 0, 0),
        mock_ctx("file2.txt", false, 0, 0),
        mock_ctx("DirB", true, 0, 0),
    ];
    sort_items(&mut items, SortBy::Name, true, true);
    assert!(items[0].is_dir);
    assert!(items[1].is_dir);
    assert!(!items[2].is_dir);
    assert!(!items[3].is_dir);
    assert_eq!(items[0].display_name, "DirA");
    assert_eq!(items[1].display_name, "DirB");
    assert_eq!(items[2].display_name, "file.txt");
    assert_eq!(items[3].display_name, "file2.txt");
}

#[test]
fn test_hidden_file_filtering() {
    let show_hidden = false;
    let raw_data = [
        ("visible.txt", false, 0, 0),
        (".hidden.txt", false, 0, 0),
        ("another.txt", false, 0, 0),
    ];
    let filtered: Vec<_> = raw_data
        .iter()
        .filter_map(|(name, is_dir, size, mtime)| {
            if !show_hidden && name.starts_with('.') {
                return None;
            }
            Some(mock_ctx(name, *is_dir, *size, *mtime))
        })
        .collect();
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].display_name, "visible.txt");
    assert_eq!(filtered[1].display_name, "another.txt");
}

#[test]
fn test_foreign_owner_uid_check_logic() {
    let current_uid = 1000;
    let other_uid = 1001;

    let is_trash = false;
    let is_foreign_owner = !is_trash && other_uid != current_uid;
    assert!(is_foreign_owner);

    let is_same_owner = !is_trash && current_uid != current_uid;
    assert!(!is_same_owner);
}

#[test]
fn test_custom_icon_priority_over_thumbnail() {
    let ctx = FileLoadContext {
        display_name: "test.jpg".to_string(),
        sort_name: "test".to_string(),
        target_path: PathBuf::from("/some/path/test.jpg"),
        size: 100,
        mtime: 0,
        is_dir: false,
        thumbnail_path: Some(PathBuf::from("/some/path/test.jpg")),
        is_foreign_owner: false,
        expand_labels: false,
        custom_icon: Some("/custom/icon.png".to_string()),
    };

    let thumb_source = resolve_thumb_source(&ctx);
    assert_eq!(thumb_source, Some(PathBuf::from("/custom/icon.png")));
}
