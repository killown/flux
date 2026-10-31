use flux::domain::model::TrashedItem;
use flux::infra::trash::tracked::{assign_new_entries, find_entry, pick_new_entry, TrashEntry};
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

// ── Batch assignment ─────────────────────────────────────────────────────

fn names(got: &[Option<TrashEntry>]) -> Vec<Option<&str>> {
    got.iter()
        .map(|e| e.as_ref().map(|e| e.name.as_str()))
        .collect()
}

#[test]
fn batch_assigns_each_path_its_own_entry() {
    let before: HashSet<String> = ["old".to_string()].into();
    let after = vec![
        entry("old", "/h/a.txt", "2026-01-01T09:00:00"),
        entry("b.txt", "/h/b.txt", "2026-01-01T10:00:01"),
        entry("a.txt", "/h/a.txt", "2026-01-01T10:00:00"),
    ];
    let paths = ["/h/a.txt".to_string(), "/h/b.txt".to_string()];
    let got = assign_new_entries(&before, after, &paths);
    assert_eq!(names(&got), [Some("a.txt"), Some("b.txt")]);
}

#[test]
fn batch_never_hands_one_entry_to_two_paths() {
    let after = vec![entry("t", "/real/t", "2026-01-01T10:00:00")];
    let paths = ["/real/t".to_string(), "/real/t".to_string()];
    let got = assign_new_entries(&HashSet::new(), after, &paths);
    assert_eq!(got.iter().flatten().count(), 1);
}

#[test]
fn batch_same_path_twice_takes_newest_first() {
    let after = vec![
        entry("a.txt", "/h/a.txt", "2026-01-01T10:00:00"),
        entry("a.2.txt", "/h/a.txt", "2026-01-01T11:00:00"),
    ];
    let paths = ["/h/a.txt".to_string(), "/h/a.txt".to_string()];
    let got = assign_new_entries(&HashSet::new(), after, &paths);
    assert_eq!(names(&got), [Some("a.2.txt"), Some("a.txt")]);
}

#[test]
fn batch_pairs_the_single_leftover_path_and_entry() {
    let after = vec![
        entry("a", "/h/a", "2026-01-01T10:00:00"),
        entry("b", "/real/b", "2026-01-01T10:00:01"),
    ];
    let paths = ["/h/a".to_string(), "/link/b".to_string()];
    let got = assign_new_entries(&HashSet::new(), after, &paths);
    assert_eq!(names(&got), [Some("a"), Some("b")]);
}

#[test]
fn batch_leaves_ambiguous_leftovers_unassigned() {
    let after = vec![
        entry("x", "/real/x", "2026-01-01T10:00:00"),
        entry("y", "/real/y", "2026-01-01T10:00:01"),
    ];
    let paths = ["/link/x".to_string(), "/link/y".to_string()];
    let got = assign_new_entries(&HashSet::new(), after, &paths);
    assert_eq!(names(&got), [None, None]);
}

#[test]
fn batch_of_one_matches_pick_new_entry() {
    let before: HashSet<String> = ["a.txt".to_string()].into();
    let after = || {
        vec![
            entry("a.txt", "/h/a.txt", "2026-01-01T10:00:00"),
            entry("a.2.txt", "/h/a.txt", "2026-01-01T11:00:00"),
        ]
    };
    let single = pick_new_entry(&before, after(), "/h/a.txt");
    let batch = assign_new_entries(&before, after(), &["/h/a.txt".to_string()]);
    assert_eq!(
        single.map(|e| e.name),
        batch.into_iter().next().flatten().map(|e| e.name)
    );
}

#[test]
fn batch_with_no_new_entries_assigns_nothing() {
    let before: HashSet<String> = ["a.txt".to_string()].into();
    let after = vec![entry("a.txt", "/h/a.txt", "2026-01-01T10:00:00")];
    let got = assign_new_entries(&before, after, &["/h/a.txt".to_string()]);
    assert_eq!(names(&got), [None]);
}
