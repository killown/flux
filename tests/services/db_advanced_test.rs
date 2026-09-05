use flux::services::db::StateManager;
use std::path::Path;
use tempfile::tempdir;

fn test_db() -> (StateManager, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_advanced_state.db");
    let mgr = StateManager::new_with_path(&db_path).unwrap();
    (mgr, dir)
}

#[test]
fn test_db_set_and_get_tags() {
    let (db, _dir) = test_db();
    let file = Path::new("/home/user/project/main.rs");

    let tags = vec!["rust".to_string(), "code".to_string(), "flux".to_string()];
    db.set_tags(file, &tags, 1700000000).unwrap();

    let retrieved = db.get_tags(file).unwrap();
    assert_eq!(retrieved, vec!["code", "flux", "rust"]); // Alphabetical order

    let paths = db.get_paths_for_tag("rust").unwrap();
    assert_eq!(paths, vec![file.to_path_buf()]);
}

#[test]
fn test_db_list_all_tags_distinct() {
    let (db, _dir) = test_db();
    let file1 = Path::new("/file1.txt");
    let file2 = Path::new("/file2.txt");

    db.set_tags(file1, &["common".into(), "first".into()], 100)
        .unwrap();
    db.set_tags(file2, &["common".into(), "second".into()], 200)
        .unwrap();

    let all_tags = db.list_all_tags().unwrap();
    assert_eq!(all_tags, vec!["common", "first", "second"]);
}

#[test]
fn test_db_delete_tag_globally() {
    let (db, _dir) = test_db();
    let file1 = Path::new("/file1.txt");
    let file2 = Path::new("/file2.txt");

    db.set_tags(file1, &["tag_to_delete".into(), "keep".into()], 100)
        .unwrap();
    db.set_tags(file2, &["tag_to_delete".into(), "keep2".into()], 200)
        .unwrap();

    assert_eq!(db.get_paths_for_tag("tag_to_delete").unwrap().len(), 2);

    db.delete_tag_globally("tag_to_delete").unwrap();

    assert_eq!(db.get_paths_for_tag("tag_to_delete").unwrap().len(), 0);
    assert_eq!(db.get_tags(file1).unwrap(), vec!["keep"]);
    assert_eq!(db.get_tags(file2).unwrap(), vec!["keep2"]);
}

#[test]
fn test_db_folder_icons_cache() {
    let (db, _dir) = test_db();
    let p = "/home/user/CustomFolder";

    assert!(db.load_folder_icons().is_empty());

    db.set_folder_icon(p, "custom-icon-symbolic").unwrap();
    let icons = db.load_folder_icons();
    assert_eq!(icons.get(p), Some(&"custom-icon-symbolic".to_string()));

    db.remove_folder_icon(p).unwrap();
    assert!(db.load_folder_icons().is_empty());
}

#[test]
fn test_db_location_history_capped() {
    let (db, _dir) = test_db();

    for i in 0..15 {
        db.add_location(&format!("smb://server/share_{}", i))
            .unwrap();
    }

    let history = db.get_location_history().unwrap();
    assert_eq!(history.len(), 15);
    // Most recent must be on top
    assert_eq!(history[0], "smb://server/share_14");
}
