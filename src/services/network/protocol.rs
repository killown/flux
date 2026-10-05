use std::path::PathBuf;

pub const NETWORK_ROOT_URI: &str = "network:///";
pub const SMB_SCHEME: &str = "smb";
pub const SFTP_SCHEME: &str = "sftp";
pub const DAV_SCHEME: &str = "dav";
pub const DAVS_SCHEME: &str = "davs";
pub const NFS_SCHEME: &str = "nfs";
pub const FTP_SCHEME: &str = "ftp";
pub const FTPS_SCHEME: &str = "ftps";
pub const MTP_SCHEME: &str = "mtp";
pub const GPHOTO2_SCHEME: &str = "gphoto2";
pub const GOOGLE_DRIVE_SCHEME: &str = "google-drive";
pub const AFP_SCHEME: &str = "afp";
pub const DNS_SD_SCHEME: &str = "dns-sd";
pub const ADMIN_SCHEME: &str = "admin";

pub const NETWORK_SCHEMES: &[&str] = &[
    NETWORK_ROOT_URI,
    "smb://",
    "sftp://",
    "dav://",
    "davs://",
    "nfs://",
    "ftp://",
    "ftps://",
    "mtp://",
    "gphoto2://",
    "google-drive://",
    "afp://",
    "dns-sd://",
    "admin://",
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NetworkProtocol {
    Smb,
    Sftp,
    WebDav,
    WebDavTls,
    Nfs,
    Ftp,
    FtpTls,
    Mtp,
    Ptp,
    GoogleDrive,
    Afp,
    DnsSd,
    Admin,
    NetworkNeighbour,
}

impl NetworkProtocol {
    pub fn default_scheme(&self) -> &'static str {
        match self {
            Self::Smb => "smb://",
            Self::Sftp => "sftp://",
            Self::WebDav => "dav://",
            Self::WebDavTls => "davs://",
            Self::Nfs => "nfs://",
            Self::Ftp => "ftp://",
            Self::FtpTls => "ftps://",
            Self::Mtp => "mtp://",
            Self::Ptp => "gphoto2://",
            Self::GoogleDrive => "google-drive://",
            Self::Afp => "afp://",
            Self::DnsSd => "dns-sd://",
            Self::Admin => "admin://",
            Self::NetworkNeighbour => "network://",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Smb => "Windows Share (SMB)",
            Self::Sftp => "SSH / SFTP",
            Self::WebDav => "WebDAV",
            Self::WebDavTls => "WebDAV (TLS)",
            Self::Nfs => "NFS",
            Self::Ftp => "FTP",
            Self::FtpTls => "FTP (TLS)",
            Self::Mtp => "MTP Device",
            Self::Ptp => "Camera (PTP)",
            Self::GoogleDrive => "Google Drive",
            Self::Afp => "AFP (Mac Share)",
            Self::DnsSd => "Network Discovery",
            Self::Admin => "Administrator Access",
            Self::NetworkNeighbour => "Network",
        }
    }

    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::Smb => "network-server-symbolic",
            Self::Sftp => "utilities-terminal-symbolic",
            Self::WebDav | Self::WebDavTls => "folder-remote-symbolic",
            Self::Nfs => "drive-harddisk-symbolic",
            Self::Ftp | Self::FtpTls => "folder-remote-symbolic",
            Self::Mtp => "phone-symbolic",
            Self::Ptp => "camera-photo-symbolic",
            Self::GoogleDrive => "drive-multidisk-symbolic",
            Self::Afp => "computer-apple-symbolic",
            Self::DnsSd => "network-wireless-symbolic",
            Self::Admin => "security-high-symbolic",
            Self::NetworkNeighbour => "network-workgroup-symbolic",
        }
    }
}

impl std::fmt::Display for NetworkProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

pub fn protocol_for_uri(uri: &str) -> Option<NetworkProtocol> {
    let scheme = uri.split("://").next()?;
    Some(match scheme {
        "smb" => NetworkProtocol::Smb,
        "sftp" | "ssh" => NetworkProtocol::Sftp,
        "dav" => NetworkProtocol::WebDav,
        "davs" => NetworkProtocol::WebDavTls,
        "nfs" => NetworkProtocol::Nfs,
        "ftp" => NetworkProtocol::Ftp,
        "ftps" => NetworkProtocol::FtpTls,
        "mtp" => NetworkProtocol::Mtp,
        "gphoto2" => NetworkProtocol::Ptp,
        "google-drive" => NetworkProtocol::GoogleDrive,
        "afp" => NetworkProtocol::Afp,
        "dns-sd" => NetworkProtocol::DnsSd,
        "admin" => NetworkProtocol::Admin,
        "network" => NetworkProtocol::NetworkNeighbour,
        _ => return None,
    })
}

#[inline]
pub fn is_network_uri(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    s == NETWORK_ROOT_URI
        || NETWORK_SCHEMES
            .iter()
            .skip(1)
            .any(|scheme| s.starts_with(scheme))
}

#[inline]
pub fn network_uri_to_path(uri: &str) -> PathBuf {
    PathBuf::from(uri)
}

#[inline]
pub fn path_to_network_uri(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}
