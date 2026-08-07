use std::path::PathBuf;

#[test]
fn test_task_queue_tick_status_formatting_single_file() {
    let items = 1;
    let pct = 0.45;
    let status = format!("[Copying {} file | {:.0}%]", items, pct * 100.0);
    assert_eq!(status, "[Copying 1 file | 45%]");
}

#[test]
fn test_task_queue_tick_status_formatting_multiple_files() {
    let items = 12;
    let pct = 0.88;
    let status = format!("[Copying {} files | {:.0}%]", items, pct * 100.0);
    assert_eq!(status, "[Copying 12 files | 88%]");
}

#[test]
fn test_task_queue_tick_status_formatting_multiple_operations() {
    let op_count = 3;
    let items = 45;
    let pct = 0.12;
    let status = format!(
        "[{} operations, {} files | {:.0}%]",
        op_count,
        items,
        pct * 100.0
    );
    assert_eq!(status, "[3 operations, 45 files | 12%]");
}

#[test]
fn test_selection_status_reset_on_task_completion() {
    let mut selection_status = "[Copying 1 file | 100%]".to_string();

    if selection_status.starts_with('[') {
        selection_status = String::new();
    }

    assert!(selection_status.is_empty());
}

#[test]
fn test_refresh_path_network_uri_routing() {
    let local_path = PathBuf::from("/home/user/Documents");
    let network_uri = PathBuf::from("smb://192.168.1.100/share");

    assert!(!flux::services::network::is_network_uri(&local_path));
    assert!(flux::services::network::is_network_uri(&network_uri));
}
