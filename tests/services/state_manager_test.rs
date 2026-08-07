use flux::services::db::StateManager;
use std::path::PathBuf;

fn create_test_db() -> (StateManager, tempfile::TempDir) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_state.db");
    let manager = StateManager::new_with_path(&db_path).expect("Failed to create test DB");
    (manager, temp_dir)
}

#[test]
fn test_save_and_get_view() {
    let (manager, _temp) = create_test_db();
    let path = PathBuf::from("/home/user/downloads");

    manager.save_view(&path, "Date", true, 64, false).unwrap();

    let result = manager.get_view(&path).unwrap().unwrap();

    assert_eq!(result.0, "Date");
    assert!(result.1);
    assert_eq!(result.2, 64);
    assert!(!result.3);
}

#[test]
fn test_get_nonexistent_view() {
    let (manager, _temp) = create_test_db();
    let path = PathBuf::from("/nonexistent/path");

    let result = manager.get_view(&path).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_update_existing_view() {
    let (manager, _temp) = create_test_db();
    let path = PathBuf::from("/home/user/Downloads");

    manager.save_view(&path, "Date", true, 64, false).unwrap();
    manager.save_view(&path, "Size", false, 256, true).unwrap();

    let result = manager.get_view(&path).unwrap().unwrap();
    assert_eq!(result.0, "Size");
    assert!(!result.1);
    assert_eq!(result.2, 256);
    assert!(result.3);
}

#[test]
fn test_rename_path() {
    let (manager, _temp) = create_test_db();
    let old_path = PathBuf::from("/home/user/OldName");
    let new_path = PathBuf::from("/home/user/NewName");

    manager
        .save_view(&old_path, "Name", false, 128, true)
        .unwrap();

    manager.rename_path(&old_path, &new_path).unwrap();

    assert!(manager.get_view(&old_path).unwrap().is_none());

    let result = manager.get_view(&new_path).unwrap().unwrap();
    assert_eq!(result.0, "Name");
}

#[test]
fn test_scrub_orphans() {
    let (manager, temp_dir) = create_test_db();

    let real_dir = temp_dir.path().join("real_folder");
    std::fs::create_dir(&real_dir).unwrap();

    let fake_dir = temp_dir.path().join("fake_folder");

    manager
        .save_view(&real_dir, "Name", false, 128, true)
        .unwrap();
    manager
        .save_view(&fake_dir, "Date", true, 64, false)
        .unwrap();

    assert!(manager.get_view(&real_dir).unwrap().is_some());
    assert!(manager.get_view(&fake_dir).unwrap().is_some());

    manager.scrub_orphans().unwrap();

    assert!(manager.get_view(&real_dir).unwrap().is_some());
    assert!(manager.get_view(&fake_dir).unwrap().is_none());
}

#[test]
fn test_location_history() {
    let (manager, _temp) = create_test_db();

    manager.add_location("smb://server/share").unwrap();
    manager.add_location("sftp://localhost").unwrap();
    manager.add_location("smb://server/share").unwrap();

    let history = manager.get_location_history().unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0], "smb://server/share");

    manager.clear_location_history().unwrap();
    let empty_history = manager.get_location_history().unwrap();
    assert!(empty_history.is_empty());
}

#[test]
fn test_remove_location() {
    let (manager, _temp) = create_test_db();

    manager.add_location("smb://nas/files").unwrap();
    manager.add_location("ftp://example.com").unwrap();

    manager.remove_location("smb://nas/files").unwrap();

    let history = manager.get_location_history().unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], "ftp://example.com");
}
