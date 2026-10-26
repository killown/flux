use flux::infra::trash::tracked::{find_entry, pick_new_entry, TrashEntry};
use flux::ui::undo_redo::TrashedItem;
use std::collections::HashSet;
use std::path::PathBuf;

fn entry(name: &str, orig: &str, date: &str) -> TrashEntry {
    TrashEntry {
        name: name.into(),
        orig_path: Some(orig.into()),
        deletion_date: Some(date.into()),
    }
}

fn item(path: &str, name: Option<&str>, date: Option<&str>) -> TrashedItem {
    TrashedItem {
        path: PathBuf::from(path),
        trash_name: name.map(Into::into),
        deletion_date: date.map(Into::into),
    }
}

#[test]
fn new_entry_is_the_one_not_present_before() {
    let before: HashSet<String> = ["a.txt".to_string()].into();
    let after = vec![
        entry("a.txt", "/h/a.txt", "2026-01-01T10:00:00"),
        entry("a.2.txt", "/h/a.txt", "2026-01-01T11:00:00"),
    ];
    let got = pick_new_entry(&before, after, "/h/a.txt").unwrap();
    assert_eq!(got.name, "a.2.txt");
}

#[test]
fn single_new_entry_is_used_when_orig_path_differs() {
    let before = HashSet::new();
    let after = vec![entry("a.txt", "/real/a.txt", "2026-01-01T10:00:00")];
    let got = pick_new_entry(&before, after, "/link/a.txt").unwrap();
    assert_eq!(got.name, "a.txt");
}

#[test]
fn ambiguous_new_entries_without_path_match_give_none() {
    let before = HashSet::new();
    let after = vec![
        entry("x", "/other/x", "2026-01-01T10:00:00"),
        entry("y", "/other/y", "2026-01-01T10:00:00"),
    ];
    assert!(pick_new_entry(&before, after, "/h/a.txt").is_none());
}

#[test]
fn find_entry_prefers_tracked_name_over_newer_same_path() {
    let entries = vec![
        entry("a.txt", "/h/a.txt", "2026-01-01T10:00:00"),
        entry("a.2.txt", "/h/a.txt", "2026-01-01T11:00:00"),
    ];
    let it = item("/h/a.txt", Some("a.txt"), Some("2026-01-01T10:00:00"));
    let got = find_entry(&entries, &it, &HashSet::new()).unwrap();
    assert_eq!(got.name, "a.txt");
}

#[test]
fn find_entry_rejects_reused_name_with_different_date() {
    let entries = vec![entry("a.txt", "/h/a.txt", "2026-02-02T10:00:00")];
    let it = item("/h/a.txt", Some("a.txt"), Some("2026-01-01T10:00:00"));
    assert!(find_entry(&entries, &it, &HashSet::new()).is_none());
}

#[test]
fn find_entry_missing_tracked_name_does_not_fall_back_to_path() {
    let entries = vec![entry("a.2.txt", "/h/a.txt", "2026-01-01T11:00:00")];
    let it = item("/h/a.txt", Some("a.txt"), None);
    assert!(find_entry(&entries, &it, &HashSet::new()).is_none());
}

#[test]
fn untracked_item_falls_back_to_newest_entry_for_path() {
    let entries = vec![
        entry("a.txt", "/h/a.txt", "2026-01-01T10:00:00"),
        entry("a.2.txt", "/h/a.txt", "2026-01-01T11:00:00"),
        entry("b.txt", "/h/b.txt", "2026-01-01T12:00:00"),
    ];
    let it = TrashedItem::untracked(PathBuf::from("/h/a.txt"));
    let got = find_entry(&entries, &it, &HashSet::new()).unwrap();
    assert_eq!(got.name, "a.2.txt");
}

#[test]
fn used_entries_are_skipped() {
    let entries = vec![
        entry("a.txt", "/h/a.txt", "2026-01-01T10:00:00"),
        entry("a.2.txt", "/h/a.txt", "2026-01-01T11:00:00"),
    ];
    let used: HashSet<String> = ["a.2.txt".to_string()].into();
    let it = TrashedItem::untracked(PathBuf::from("/h/a.txt"));
    let got = find_entry(&entries, &it, &used).unwrap();
    assert_eq!(got.name, "a.txt");
}
