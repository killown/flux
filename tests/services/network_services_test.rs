use flux::services::network::{
    is_network_uri, protocol_for_uri, ConnectToServerParams, NetworkAuthFlags, NetworkBookmark,
    NetworkCredentials, NetworkProtocol,
};
use std::path::PathBuf;

#[test]
fn test_is_network_uri_parsing() {
    assert!(is_network_uri(&PathBuf::from("smb://server/share")));
    assert!(is_network_uri(&PathBuf::from("sftp://user@host/path")));
    assert!(is_network_uri(&PathBuf::from("dav://server/dav")));
    assert!(is_network_uri(&PathBuf::from("nfs://server/export")));
    assert!(is_network_uri(&PathBuf::from("ftp://ftp.example.com")));
    assert!(is_network_uri(&PathBuf::from("mtp://[usb:001,002]/")));
    assert!(is_network_uri(&PathBuf::from("google-drive://user/")));
    assert!(is_network_uri(&PathBuf::from("afp://server/share")));
    assert!(is_network_uri(&PathBuf::from("network:///")));

    assert!(!is_network_uri(&PathBuf::from("/home/user")));
    assert!(!is_network_uri(&PathBuf::from("trash://")));
    assert!(!is_network_uri(&PathBuf::from("/archive://something")));
}

#[test]
fn test_protocol_for_uri_matching() {
    assert_eq!(protocol_for_uri("smb://server"), Some(NetworkProtocol::Smb));
    assert_eq!(protocol_for_uri("sftp://host"), Some(NetworkProtocol::Sftp));
    assert_eq!(protocol_for_uri("ssh://host"), Some(NetworkProtocol::Sftp));
    assert_eq!(
        protocol_for_uri("dav://host"),
        Some(NetworkProtocol::WebDav)
    );
    assert_eq!(
        protocol_for_uri("davs://host"),
        Some(NetworkProtocol::WebDavTls)
    );
    assert_eq!(protocol_for_uri("nfs://host"), Some(NetworkProtocol::Nfs));
    assert_eq!(protocol_for_uri("ftp://host"), Some(NetworkProtocol::Ftp));
    assert_eq!(
        protocol_for_uri("ftps://host"),
        Some(NetworkProtocol::FtpTls)
    );
    assert_eq!(protocol_for_uri("mtp://device"), Some(NetworkProtocol::Mtp));
    assert_eq!(
        protocol_for_uri("gphoto2://camera"),
        Some(NetworkProtocol::Ptp)
    );
    assert_eq!(
        protocol_for_uri("google-drive://user"),
        Some(NetworkProtocol::GoogleDrive)
    );
    assert_eq!(protocol_for_uri("afp://server"), Some(NetworkProtocol::Afp));
    assert_eq!(
        protocol_for_uri("admin:///etc"),
        Some(NetworkProtocol::Admin)
    );
    assert_eq!(protocol_for_uri("unknown://host"), None);
}

#[test]
fn test_connect_params_build_uri_smb() {
    let params = ConnectToServerParams {
        protocol: "smb".into(),
        host: "server".into(),
        port: None,
        path: Some("share".into()),
        username: Some("user".into()),
    };
    assert_eq!(
        params.build_uri(),
        Some("smb://user@server/share".to_string())
    );
}

#[test]
fn test_connect_params_build_uri_sftp_with_port() {
    let params = ConnectToServerParams {
        protocol: "sftp".into(),
        host: "192.168.1.10".into(),
        port: Some(2222),
        path: None,
        username: None,
    };
    assert_eq!(
        params.build_uri(),
        Some("sftp://192.168.1.10:2222/".to_string())
    );
}

#[test]
fn test_connect_params_empty_host_returns_none() {
    let params = ConnectToServerParams {
        protocol: "smb".into(),
        host: "".into(),
        ..Default::default()
    };
    assert!(params.build_uri().is_none());
}

#[test]
fn test_connect_params_unknown_protocol_returns_none() {
    let params = ConnectToServerParams {
        protocol: "xyz".into(),
        host: "host".into(),
        ..Default::default()
    };
    assert!(params.build_uri().is_none());
}

#[test]
fn test_network_bookmark_infers_icon() {
    let b = NetworkBookmark::new("My NAS", "smb://nas/media");
    assert_eq!(b.icon, "network-server-symbolic");

    let b = NetworkBookmark::new("Remote Dev", "sftp://dev.example.com/");
    assert_eq!(b.icon, "utilities-terminal-symbolic");
}

#[test]
fn test_credentials_anonymous() {
    let creds = NetworkCredentials::anonymous();
    assert!(creds.anonymous);
    assert!(creds.username.is_none());
}

#[test]
fn test_credentials_with_password() {
    let creds = NetworkCredentials::with_password("alice", "s3cr3t");
    assert_eq!(creds.username.as_deref(), Some("alice"));
    assert_eq!(creds.password.as_deref(), Some("s3cr3t"));
    assert!(!creds.anonymous);
}

#[test]
fn test_network_auth_flags_bits() {
    let flags = NetworkAuthFlags::USERNAME | NetworkAuthFlags::PASSWORD;
    assert!(flags.contains(NetworkAuthFlags::USERNAME));
    assert!(flags.contains(NetworkAuthFlags::PASSWORD));
    assert!(!flags.contains(NetworkAuthFlags::DOMAIN));
}

#[test]
fn test_classify_enum_error_mapping() {
    let perm_err =
        gtk::glib::Error::new(gtk::gio::IOErrorEnum::PermissionDenied, "Permission denied");
    let net_err = flux::services::network::NetworkError::from(perm_err);
    assert!(matches!(
        net_err,
        flux::services::network::NetworkError::AuthFailed
    ));

    let host_err = gtk::glib::Error::new(gtk::gio::IOErrorEnum::HostNotFound, "Host unreachable");
    let net_err2 = flux::services::network::NetworkError::from(host_err);
    assert!(matches!(
        net_err2,
        flux::services::network::NetworkError::HostUnreachable(_)
    ));
}

#[test]
fn test_uri_display_name_stripping() {
    let uri_display_name = |uri: &str| -> String {
        let without_scheme = uri.split("://").nth(1).unwrap_or(uri).trim_end_matches('/');
        if without_scheme.is_empty() {
            return "".to_owned();
        }
        without_scheme.to_owned()
    };

    assert_eq!(
        uri_display_name("smb://192.168.1.1/share/"),
        "192.168.1.1/share"
    );
    assert_eq!(uri_display_name("sftp://example.com/"), "example.com");
    assert_eq!(uri_display_name("smb://"), "");
}
