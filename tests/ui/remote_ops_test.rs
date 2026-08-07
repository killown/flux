use flux::model::Config;
use flux::services::network::NetworkBookmark;

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
