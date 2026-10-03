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

#[test]
fn db_empty_tags_list_clears_all() {
    let (db, _dir) = test_db();
    let p = Path::new("/x");
    db.set_tags(p, &["a".into(), "b".into()], 1).unwrap();
    db.set_tags(p, &[], 2).unwrap();
    assert!(db.get_tags(p).unwrap().is_empty());
}

#[test]
fn db_tag_with_hash_stripped_in_storage() {
    let (db, _dir) = test_db();
    let p = Path::new("/x");
    db.set_tags(p, &["#rust".into()], 1).unwrap();
    assert_eq!(db.get_tags(p).unwrap(), vec!["rust"]);
}

#[test]
fn db_paths_for_tag_empty_when_none_match() {
    let (db, _dir) = test_db();
    assert!(db.get_paths_for_tag("nonexistent").unwrap().is_empty());
}

#[test]
fn db_list_all_tags_empty_on_fresh() {
    let (db, _dir) = test_db();
    assert!(db.list_all_tags().unwrap().is_empty());
}

#[test]
fn db_folder_icon_overwrite() {
    let (db, _dir) = test_db();
    db.set_folder_icon("/p", "icon-a").unwrap();
    db.set_folder_icon("/p", "icon-b").unwrap();
    let icons = db.load_folder_icons();
    assert_eq!(icons.get("/p"), Some(&"icon-b".to_string()));
}

#[test]
fn db_remove_folder_icon_missing_is_ok() {
    let (db, _dir) = test_db();
    assert!(db.remove_folder_icon("/never-existed").is_ok());
}

#[test]
fn db_location_history_dedup_moves_to_top() {
    let (db, _dir) = test_db();
    db.add_location("a").unwrap();
    db.add_location("b").unwrap();
    db.add_location("a").unwrap();
    let h = db.get_location_history().unwrap();
    assert_eq!(h.len(), 2);
    assert_eq!(h[0], "a");
}

#[test]
fn db_clear_location_history_on_empty_is_ok() {
    let (db, _dir) = test_db();
    assert!(db.clear_location_history().is_ok());
}

#[test]
fn rekey_path_prefix_moves_self_and_children() {
    use flux::services::db::rekey_path_prefix;
    use std::collections::HashMap;

    let mut m: HashMap<String, u32> = HashMap::new();
    m.insert("/home/u/Old".into(), 1);
    m.insert("/home/u/Old/child.png".into(), 2);
    m.insert("/home/u/Old/deep/x".into(), 3);
    m.insert("/home/u/Oldish".into(), 4);
    m.insert("/home/u/Other".into(), 5);

    assert!(rekey_path_prefix(&mut m, "/home/u/Old", "/home/u/New"));

    assert_eq!(m.get("/home/u/New"), Some(&1));
    assert_eq!(m.get("/home/u/New/child.png"), Some(&2));
    assert_eq!(m.get("/home/u/New/deep/x"), Some(&3));
    assert_eq!(m.get("/home/u/Oldish"), Some(&4));
    assert_eq!(m.get("/home/u/Other"), Some(&5));
    assert_eq!(m.len(), 5);
}

#[test]
fn rekey_path_prefix_no_op_on_self_or_empty() {
    use flux::services::db::rekey_path_prefix;
    use std::collections::HashMap;
    let mut m: HashMap<String, u32> = HashMap::new();
    m.insert("/a".into(), 1);
    assert!(!rekey_path_prefix(&mut m, "/a", "/a"));
    assert!(!rekey_path_prefix(&mut m, "", "/b"));
    assert!(!rekey_path_prefix(&mut m, "/b", ""));
}
