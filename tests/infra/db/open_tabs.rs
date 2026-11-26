use flux::infra::db::StateManager;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn fresh_db_has_no_saved_tabs() {
    let dir = tempdir().unwrap();
    let db = StateManager::new_with_path(&dir.path().join("s.db")).unwrap();
    assert_eq!(db.load_open_tabs(), (Vec::new(), 0));
}

#[test]
fn saved_tabs_round_trip_in_order_with_active_index() {
    let dir = tempdir().unwrap();
    let db = StateManager::new_with_path(&dir.path().join("s.db")).unwrap();
    let tabs: Vec<PathBuf> = ["/home/a", "/srv/b b", "/tmp/c"]
        .iter()
        .map(PathBuf::from)
        .collect();
    db.save_open_tabs(&tabs, 2).unwrap();
    assert_eq!(db.load_open_tabs(), (tabs, 2));
}

#[test]
fn saving_again_replaces_the_previous_tabs() {
    let dir = tempdir().unwrap();
    let db = StateManager::new_with_path(&dir.path().join("s.db")).unwrap();
    db.save_open_tabs(&[PathBuf::from("/a"), PathBuf::from("/b")], 1)
        .unwrap();
    db.save_open_tabs(&[PathBuf::from("/c")], 0).unwrap();
    assert_eq!(db.load_open_tabs(), (vec![PathBuf::from("/c")], 0));
}
