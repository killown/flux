use flux::domain::model::quick_groups::{NameError, QuickGroup, QuickGroups, DEFAULT_NAME};
use flux::domain::model::quick_list as ql;
use std::path::{Path, PathBuf};

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

fn items(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(|s| p(s)).collect()
}

/// Two lists: "Work" = /w1 /w2 /w3 (selected, slot 0), "Photos" = /p1.
fn sample() -> QuickGroups {
    let mut g = QuickGroups::single("Work", items(&["/w1", "/w2", "/w3"]), Some(0));
    let i = g.add_list("Photos").unwrap();
    g.add_to(i, [p("/p1")]);
    assert!(g.invariant_holds());
    g
}

fn names(g: &QuickGroups) -> Vec<&str> {
    g.lists.iter().map(|l| l.name.as_str()).collect()
}

#[test]
fn single_clamps_slot_and_names_the_list() {
    let g = QuickGroups::single("  ", items(&["/a", "/b"]), Some(9));
    assert_eq!(g.lists[0].name, DEFAULT_NAME);
    assert_eq!(g.lists[0].index, Some(0));
    let e = QuickGroups::single("x", vec![], Some(3));
    assert_eq!(e.lists[0].index, None);
    assert!(g.invariant_holds() && e.invariant_holds());
}

#[test]
fn names_are_trimmed_unique_ignoring_case_and_non_empty() {
    let mut g = sample();
    assert_eq!(g.add_list("   "), Err(NameError::Empty));
    assert_eq!(g.add_list("work"), Err(NameError::Taken));
    assert_eq!(g.add_list("  Docs  "), Ok(2));
    assert_eq!(names(&g), ["Work", "Photos", "Docs"]);
    assert_eq!(g.rename(1, "WORK"), Err(NameError::Taken));
    assert_eq!(g.rename(1, ""), Err(NameError::Empty));
    assert_eq!(g.rename(1, "Photos"), Ok(false));
    assert_eq!(
        g.rename(1, "photos"),
        Ok(true),
        "case-only change is allowed"
    );
    assert_eq!(g.rename(7, "Nope"), Ok(false));
    assert!(g.invariant_holds());
}

#[test]
fn unique_name_counts_up() {
    let mut g = sample();
    assert_eq!(g.unique_name("New list"), "New list");
    g.add_list("New list").unwrap();
    assert_eq!(g.unique_name("New list"), "New list 2");
    g.add_list("New list 2").unwrap();
    assert_eq!(g.unique_name("new list"), "new list 3");
}

#[test]
fn add_list_does_not_move_the_selection() {
    let mut g = sample();
    g.add_list("Docs").unwrap();
    assert_eq!(g.selected, 0);
}

#[test]
fn select_reports_change() {
    let mut g = sample();
    assert!(!g.select(0));
    assert!(!g.select(5));
    assert!(g.select(1));
    assert_eq!(g.selected, 1);
}

#[test]
fn delete_keeps_the_selection_valid_and_the_last_list() {
    let mut g = sample();
    g.add_list("Docs").unwrap();
    g.select(1);
    assert!(g.delete(0), "deleting before the selection shifts it");
    assert_eq!(g.selected, 0);
    assert_eq!(g.selected_list().name, "Photos");
    assert!(g.delete(0), "deleting the selected list selects the next");
    assert_eq!(g.selected_list().name, "Docs");
    assert!(!g.delete(0), "the last list stays");
    assert!(!g.delete(9));
    assert!(g.invariant_holds());

    let mut h = sample();
    h.select(1);
    assert!(
        h.delete(1),
        "deleting the last selected list selects the one before"
    );
    assert_eq!(h.selected, 0);
    assert!(h.invariant_holds());
}

#[test]
fn add_to_front_dedupes_inside_one_list_only() {
    let mut g = sample();
    assert_eq!(g.add_to(0, [p("/w2"), p("/new")]), 1);
    assert_eq!(g.lists[0].items, items(&["/new", "/w1", "/w2", "/w3"]));
    assert_eq!(g.lists[0].index, Some(1), "slot keeps pointing at /w1");
    assert_eq!(
        g.add_to(1, [p("/w1")]),
        1,
        "another list may hold the same folder"
    );
    assert_eq!(g.add_to(9, [p("/x")]), 0);
    assert!(g.invariant_holds());
}

#[test]
fn remove_from_matches_the_old_dock_rule() {
    let mut g = sample();
    assert!(g.remove_from(0, Path::new("/w2")));
    assert_eq!(g.lists[0].items, items(&["/w1", "/w3"]));
    assert_eq!(g.lists[0].index, Some(0));
    assert!(!g.remove_from(0, Path::new("/w2")));
    assert!(g.remove_from(1, Path::new("/p1")));
    assert_eq!(g.lists[1].index, None, "an emptied list has no slot");
    assert!(g.invariant_holds());
}

#[test]
fn move_item_merges_and_refuses_nonsense() {
    let mut g = sample();
    assert!(g.move_item(Path::new("/w2"), 0, 1));
    assert_eq!(g.lists[0].items, items(&["/w1", "/w3"]));
    assert_eq!(g.lists[1].items, items(&["/w2", "/p1"]));
    assert!(!g.move_item(Path::new("/w1"), 0, 0));
    assert!(!g.move_item(Path::new("/w1"), 0, 9));
    assert!(!g.move_item(Path::new("/missing"), 0, 1));
    g.add_to(1, [p("/w1")]);
    assert!(
        g.move_item(Path::new("/w1"), 0, 1),
        "merge with the copy already there"
    );
    assert_eq!(
        g.lists[1].items.iter().filter(|x| **x == p("/w1")).count(),
        1
    );
    assert!(g.invariant_holds());
}

#[test]
fn replace_slot_refuses_duplicates_and_bad_slots() {
    let mut g = sample();
    assert!(g.replace_slot(0, 1, p("/elsewhere")));
    assert_eq!(g.lists[0].items, items(&["/w1", "/elsewhere", "/w3"]));
    assert_eq!(g.lists[0].index, Some(1));
    assert!(!g.replace_slot(0, 2, p("/w1")));
    assert!(!g.replace_slot(0, 3, p("/x")));
    assert!(!g.replace_slot(5, 0, p("/x")));
}

#[test]
fn open_item_selects_the_list_and_points_the_slot() {
    let mut g = sample();
    assert!(g.open_item(1, Path::new("/p1")));
    assert_eq!((g.selected, g.lists[1].index), (1, Some(0)));
    assert!(g.open_item(0, Path::new("/w3")));
    assert_eq!((g.selected, g.lists[0].index), (0, Some(2)));
    assert!(!g.open_item(1, Path::new("/w3")));
    assert_eq!(g.selected, 0, "a refused open changes nothing");
}

#[test]
fn clear_list_keeps_the_list() {
    let mut g = sample();
    assert!(g.clear_list(0));
    assert_eq!((g.lists[0].items.len(), g.lists[0].index), (0, None));
    assert!(!g.clear_list(0));
    assert!(g.invariant_holds());
}

#[test]
fn with_selected_overlays_only_the_selected_list() {
    let g = sample();
    let live = items(&["/live"]);
    let o = g.with_selected(&live, Some(0));
    assert_eq!(o.lists[0].items, live);
    assert_eq!(o.lists[1], g.lists[1]);
    assert_eq!(g.lists[0].items.len(), 3, "the original is untouched");
}

#[test]
fn tab_walks_only_the_selected_list() {
    let mut g = sample();
    let sel = g.selected_list().clone();
    let mut idx = sel.index;
    let mut seen = vec![];
    for _ in 0..4 {
        let t = ql::tab(&sel.items, &mut idx, Path::new("/somewhere")).unwrap();
        seen.push(t);
        // pretend we arrived there
        let _ = ql::step(&sel.items, &mut idx, ql::Step::Next);
    }
    assert!(seen.iter().all(|s| sel.items.contains(s)));
    assert!(!seen.contains(&p("/p1")));
    g.select(1);
    assert_eq!(g.selected_list().items, items(&["/p1"]));
}

#[test]
fn views_without_a_filter_show_everything_even_empty_lists() {
    let mut g = sample();
    g.add_list("Empty").unwrap();
    let v = g.views(|_| true, |p| p.to_string_lossy().into_owned());
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].rows.len(), 3);
    assert!(v[0].selected && !v[1].selected);
    assert_eq!((v[2].total, v[2].rows.len()), (0, 0));
    assert_eq!(v[0].current, Some(0));
}

#[test]
fn views_filter_by_item_or_by_list_name() {
    let g = sample();
    let text = |p: &Path| p.to_string_lossy().into_owned();

    let v = g.views(|t| t.contains("w2"), text);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].name, "Work");
    assert_eq!(v[0].rows, vec![(1, p("/w2"))]);
    assert_eq!(v[0].total, 3, "total counts hidden rows too");

    let v = g.views(|t| t == "Photos", text);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].rows.len(), 1, "a list-name hit shows all its items");

    assert!(g.views(|t| t.contains("zzz"), text).is_empty());
}

#[test]
fn normalize_repairs_what_a_bad_database_can_hold() {
    let mut g = QuickGroups {
        lists: vec![
            QuickGroup {
                name: " A ".into(),
                items: items(&["/a"]),
                index: Some(7),
            },
            QuickGroup {
                name: "a".into(),
                items: vec![],
                index: Some(2),
            },
            QuickGroup {
                name: "".into(),
                items: items(&["/c", "/d"]),
                index: None,
            },
        ],
        selected: 9,
    };
    assert!(!g.invariant_holds());
    g.normalize();
    assert!(g.invariant_holds());
    assert_eq!(names(&g), ["A", "a 2", DEFAULT_NAME]);
    assert_eq!(g.lists[0].index, Some(0));
    assert_eq!(g.lists[1].index, None);
    assert_eq!(g.lists[2].index, Some(0));
    assert_eq!(g.selected, 2);

    let mut empty = QuickGroups {
        lists: vec![],
        selected: 3,
    };
    empty.normalize();
    assert!(empty.invariant_holds());
}
