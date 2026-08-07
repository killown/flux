use flux::services::network::{ConnectToServerParams, NetworkAuthFlags, NetworkCredentials};

#[test]
fn test_connect_to_server_uri_builder() {
    let params = ConnectToServerParams {
        protocol: "sftp".to_string(),
        host: "server.example.com".to_string(),
        port: Some(2222),
        path: Some("data/files".to_string()),
        username: Some("admin".to_string()),
    };

    let uri = params.build_uri().unwrap();
    assert_eq!(uri, "sftp://admin@server.example.com:2222/data/files");
}

#[test]
fn test_connect_to_server_default_port_uri_builder() {
    let params = ConnectToServerParams {
        protocol: "smb".to_string(),
        host: "192.168.1.100".to_string(),
        port: None,
        path: Some("share".to_string()),
        username: None,
    };

    let uri = params.build_uri().unwrap();
    assert_eq!(uri, "smb://192.168.1.100/share");
}

#[test]
fn test_network_auth_flags_bits() {
    let flags = NetworkAuthFlags::USERNAME | NetworkAuthFlags::PASSWORD;

    assert!(flags.contains(NetworkAuthFlags::USERNAME));
    assert!(flags.contains(NetworkAuthFlags::PASSWORD));
    assert!(!flags.contains(NetworkAuthFlags::DOMAIN));
    assert!(!flags.contains(NetworkAuthFlags::ANON_OK));
}

#[test]
fn test_anonymous_credentials_creation() {
    let creds = NetworkCredentials::anonymous();

    assert!(creds.anonymous);
    assert!(creds.username.is_none());
    assert!(creds.password.is_none());
    assert!(creds.domain.is_none());
}
