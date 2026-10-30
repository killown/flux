use flux::domain::model::quick_groups::QuickGroups;
use flux::infra::db::StateManager;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use tempfile::tempdir;

fn items(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

#[test]
fn fresh_db_gives_one_empty_default_list() {
    let dir = tempdir().unwrap();
    let db = StateManager::new_with_path(&dir.path().join("s.db")).unwrap();
    let g = db.load_quick_groups("Quick List");
    assert_eq!(g, QuickGroups::single("Quick List", vec![], None));
    assert!(g.invariant_holds());
}

#[test]
fn legacy_flat_list_becomes_the_first_list() {
    let dir = tempdir().unwrap();
    let db = StateManager::new_with_path(&dir.path().join("s.db")).unwrap();
    db.save_quick_list(&items(&["/c", "/b", "/a"]), Some(1))
        .unwrap();
    let g = db.load_quick_groups("Mine");
    assert_eq!(g.lists.len(), 1);
    assert_eq!(g.lists[0].name, "Mine");
    assert_eq!(g.lists[0].items, items(&["/c", "/b", "/a"]));
    assert_eq!(g.lists[0].index, Some(1));
}

#[test]
fn restart_resumes_every_list_slot_and_selection() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("s.db");
    let mut g = QuickGroups::single("Work", items(&["/w1", "/w2"]), Some(1));
    let i = g.add_list("Photos").unwrap();
    g.add_to(i, [PathBuf::from("/p1")]);
    g.add_list("Empty").unwrap();
    g.select(1);
    {
        let db = StateManager::new_with_path(&path).unwrap();
        db.save_quick_groups(&g).unwrap();
    }
    let db = StateManager::new_with_path(&path).unwrap();
    assert_eq!(db.load_quick_groups("x"), g);
}

#[test]
fn saved_lists_win_over_the_legacy_table() {
    let dir = tempdir().unwrap();
    let db = StateManager::new_with_path(&dir.path().join("s.db")).unwrap();
    db.save_quick_list(&items(&["/old"]), Some(0)).unwrap();
    let g = QuickGroups::single("New", items(&["/new"]), Some(0));
    db.save_quick_groups(&g).unwrap();
    assert_eq!(db.load_quick_groups("x"), g);
}

#[test]
fn shrinking_leaves_no_stale_rows() {
    let dir = tempdir().unwrap();
    let db = StateManager::new_with_path(&dir.path().join("s.db")).unwrap();
    let mut g = QuickGroups::single("A", items(&["/a", "/b", "/c"]), Some(2));
    g.add_list("B").unwrap();
    db.save_quick_groups(&g).unwrap();
    let small = QuickGroups::single("A", items(&["/a"]), Some(0));
    db.save_quick_groups(&small).unwrap();
    assert_eq!(db.load_quick_groups("x"), small);
}

#[test]
fn non_utf8_path_round_trips() {
    let dir = tempdir().unwrap();
    let db = StateManager::new_with_path(&dir.path().join("s.db")).unwrap();
    let weird = PathBuf::from(OsString::from_vec(b"/tmp/\xff\xfe".to_vec()));
    let g = QuickGroups::single("A", vec![weird.clone()], Some(0));
    db.save_quick_groups(&g).unwrap();
    assert_eq!(db.load_quick_groups("x").lists[0].items, vec![weird]);
}
