use flux::domain::model::quick_list as ql;
use flux::infra::db::StateManager;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[test]
fn fresh_db_is_empty() {
    let dir = tempdir().unwrap();
    let db = StateManager::new_with_path(&dir.path().join("s.db")).unwrap();
    assert_eq!(db.load_quick_list(), (vec![], None));
}

#[test]
fn restart_resumes_same_order_and_slot() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("s.db");
    let list: Vec<PathBuf> = ["/c", "/b", "/a"].iter().map(PathBuf::from).collect();
    {
        let db = StateManager::new_with_path(&path).unwrap();
        db.save_quick_list(&list, Some(1)).unwrap();
    }
    let db = StateManager::new_with_path(&path).unwrap();
    let (loaded, slot) = db.load_quick_list();
    assert_eq!(loaded, list);
    let mut idx = ql::restore(&loaded, slot);
    assert_eq!(idx, Some(1));
    assert_eq!(
        ql::tab(&loaded, &mut idx, Path::new("/home")),
        Some(PathBuf::from("/b"))
    );
}

#[test]
fn shrinking_and_clearing_leave_no_stale_rows() {
    let dir = tempdir().unwrap();
    let db = StateManager::new_with_path(&dir.path().join("s.db")).unwrap();
    let three: Vec<PathBuf> = ["/a", "/b", "/c"].iter().map(PathBuf::from).collect();
    db.save_quick_list(&three, Some(2)).unwrap();
    db.save_quick_list(&three[..1], Some(0)).unwrap();
    assert_eq!(db.load_quick_list(), (vec![PathBuf::from("/a")], Some(0)));
    db.save_quick_list(&[], None).unwrap();
    assert_eq!(db.load_quick_list(), (vec![], None));
}

#[test]
fn non_utf8_path_round_trips() {
    let dir = tempdir().unwrap();
    let db = StateManager::new_with_path(&dir.path().join("s.db")).unwrap();
    let odd = PathBuf::from(OsString::from_vec(b"/t/\xff\xfe".to_vec()));
    db.save_quick_list(&[odd.clone()], Some(0)).unwrap();
    assert_eq!(db.load_quick_list().0, vec![odd]);
}
