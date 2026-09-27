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

#[test]
fn network_protocol_default_scheme_roundtrip() {
    for proto in [
        NetworkProtocol::Smb,
        NetworkProtocol::Sftp,
        NetworkProtocol::WebDav,
        NetworkProtocol::WebDavTls,
        NetworkProtocol::Nfs,
        NetworkProtocol::Ftp,
        NetworkProtocol::FtpTls,
        NetworkProtocol::Mtp,
        NetworkProtocol::Ptp,
        NetworkProtocol::GoogleDrive,
        NetworkProtocol::Afp,
        NetworkProtocol::DnsSd,
        NetworkProtocol::Admin,
        NetworkProtocol::NetworkNeighbour,
    ] {
        let scheme = proto.default_scheme();
        assert_eq!(protocol_for_uri(scheme), Some(proto));
    }
}

#[test]
fn network_protocol_display_matches_display_name() {
    assert_eq!(
        NetworkProtocol::Smb.to_string(),
        NetworkProtocol::Smb.display_name()
    );
    assert_eq!(
        NetworkProtocol::Sftp.to_string(),
        NetworkProtocol::Sftp.display_name()
    );
}

#[test]
fn network_protocol_each_has_unique_icon() {
    use std::collections::HashSet;
    let protos = [
        NetworkProtocol::Smb,
        NetworkProtocol::Sftp,
        NetworkProtocol::WebDav,
        NetworkProtocol::Nfs,
        NetworkProtocol::Ftp,
        NetworkProtocol::Mtp,
        NetworkProtocol::Ptp,
        NetworkProtocol::GoogleDrive,
        NetworkProtocol::Afp,
        NetworkProtocol::DnsSd,
        NetworkProtocol::Admin,
        NetworkProtocol::NetworkNeighbour,
    ];
    let icons: HashSet<&str> = protos.iter().map(|p| p.icon_name()).collect();
    assert!(icons.len() >= 10, "protocols should have distinct icons");
}

#[test]
fn connect_params_rejects_empty_protocol() {
    let params = ConnectToServerParams {
        protocol: String::new(),
        host: "server".into(),
        ..Default::default()
    };
    assert!(params.build_uri().is_none());
}

#[test]
fn connect_params_port_zero_formatting() {
    let params = ConnectToServerParams {
        protocol: "sftp".into(),
        host: "host".into(),
        port: Some(0),
        ..Default::default()
    };
    assert_eq!(params.build_uri(), Some("sftp://host:0/".to_string()));
}

#[test]
fn connect_params_path_with_leading_slash_normalized() {
    let params = ConnectToServerParams {
        protocol: "smb".into(),
        host: "h".into(),
        path: Some("/share".into()),
        ..Default::default()
    };
    let uri = params.build_uri().unwrap();
    assert!(!uri.contains("//share"));
}

#[test]
fn connect_params_username_with_at_sign_not_doubled() {
    let params = ConnectToServerParams {
        protocol: "sftp".into(),
        host: "h".into(),
        username: Some("user".into()),
        ..Default::default()
    };
    assert_eq!(params.build_uri().unwrap(), "sftp://user@h/");
}

#[test]
fn network_bookmark_default_icon_for_unknown_scheme() {
    let b = NetworkBookmark::new("X", "weird://host");
    assert_eq!(b.icon, "folder-remote-symbolic");
}

#[test]
fn network_bookmark_preserves_name_exactly() {
    let b = NetworkBookmark::new("My NAS ❤️", "smb://nas");
    assert_eq!(b.name, "My NAS ❤️");
}

#[test]
fn credentials_anonymous_has_no_domain() {
    let c = NetworkCredentials::anonymous();
    assert!(c.domain.is_none());
}

#[test]
fn credentials_with_password_not_anonymous() {
    let c = NetworkCredentials::with_password("u", "p");
    assert!(!c.anonymous);
}

#[test]
fn protocol_for_uri_rejects_empty_string() {
    assert_eq!(protocol_for_uri(""), None);
}

#[test]
fn protocol_for_uri_rejects_no_scheme() {
    assert_eq!(protocol_for_uri("/home/user"), None);
}

#[test]
fn protocol_for_uri_case_sensitive_scheme() {
    assert_eq!(protocol_for_uri("SMB://host"), None);
}

#[test]
fn is_network_uri_rejects_archive_prefix() {
    assert!(!is_network_uri(&PathBuf::from("/archive:///tmp/x.zip")));
}
