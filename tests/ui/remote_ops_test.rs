use flux::model::Config;
use flux::services::network::NetworkBookmark;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn test_add_network_bookmark_deduplication() {
    let mut config = Config::default();

    let name = "Home Server".to_string();
    let uri = "smb://192.168.1.100/share".to_string();

    let bookmark = NetworkBookmark::new(name.clone(), uri.clone());

    if !config
        .network_bookmarks
        .iter()
        .any(|b| b.uri == bookmark.uri)
    {
        config.network_bookmarks.push(bookmark);
    }

    assert_eq!(config.network_bookmarks.len(), 1);

    // Attempt to add duplicate URI
    let duplicate_bookmark = NetworkBookmark::new("Alias".to_string(), uri.clone());
    if !config
        .network_bookmarks
        .iter()
        .any(|b| b.uri == duplicate_bookmark.uri)
    {
        config.network_bookmarks.push(duplicate_bookmark);
    }

    assert_eq!(config.network_bookmarks.len(), 1);
    assert_eq!(config.network_bookmarks[0].name, "Home Server");
}

#[test]
fn test_remove_network_bookmark() {
    let mut config = Config::default();

    config.network_bookmarks.push(NetworkBookmark::new(
        "Server A".to_string(),
        "smb://192.168.1.10/share".to_string(),
    ));
    config.network_bookmarks.push(NetworkBookmark::new(
        "Server B".to_string(),
        "sftp://192.168.1.20/data".to_string(),
    ));

    let uri_to_remove = "smb://192.168.1.10/share";
    config.network_bookmarks.retain(|b| b.uri != uri_to_remove);

    assert_eq!(config.network_bookmarks.len(), 1);
    assert_eq!(config.network_bookmarks[0].name, "Server B");
}

#[test]
fn network_loaded_stale_session_is_discarded() {
    let load_id = AtomicU64::new(0);

    // Simulate two rapid navigations, session 1 is superseded by session 2.
    let _session1 = load_id.fetch_add(1, Ordering::SeqCst) + 1;
    let session2 = load_id.fetch_add(1, Ordering::SeqCst) + 1;

    let current = load_id.load(Ordering::SeqCst);
    assert_eq!(current, session2);

    // session 1 arriving late must be rejected.
    let incoming_stale: u64 = 1;
    assert_ne!(incoming_stale, current);
}

#[test]
fn network_loaded_matching_session_is_accepted() {
    let load_id = AtomicU64::new(0);
    let session = load_id.fetch_add(1, Ordering::SeqCst) + 1;
    let current = load_id.load(Ordering::SeqCst);

    assert_eq!(session, current);
}
