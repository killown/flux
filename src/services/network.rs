// FIXME: Right-click does not select the item on SMB/network shares.
// The path retrieval via widget.data() or widget.widget_name() fails
// due to GTK widget recycling in the factory. Even with Rc<RefCell<PathBuf>>
// stored in the model and updated in bind(), the gesture closure sometimes
// reads a stale/None value when navigating rapidly or on first load.
// Possible causes: async loading triggers unbind/rebind before the gesture
// fires, or the gesture target is not the root widget where the data is stored.
// Workaround attempts: using widget name fallback, removing duplicate handler
// in grid_scroller, forcing grid refresh. Still not reliable for all cases.
// Consider rewriting the right‑click selection logic to use the selection model
// directly or pass the index via the gesture closure (capture index at bind time).

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use gtk::gio;
use gtk::prelude::*;

use crate::model::FileLoadContext;

/// Set `FLUX_DEBUG_NETWORK=1` to enable per-entry tracing on stderr.
/// Off by default so a slow mount doesn't drown the journal in
/// thousands of debug lines on every navigation.
fn net_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("FLUX_DEBUG_NETWORK").is_some())
}

macro_rules! net_debug {
    ($($arg:tt)*) => {
        if net_debug_enabled() {
            eprintln!($($arg)*);
        }
    };
}

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

#[derive(Debug)]
pub enum NetworkError {
    CredentialsRequired {
        message: String,
        flags: NetworkAuthFlags,
    },
    AuthFailed,
    HostUnreachable(String),
    GvfsUnavailable(String),
    Other(String),
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NetworkAuthFlags: u8 {
        const USERNAME = 0b0001;
        const PASSWORD = 0b0010;
        const DOMAIN   = 0b0100;
        const ANON_OK  = 0b1000;
    }
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CredentialsRequired { message, .. } => {
                write!(f, "{}: {message}", crate::i18n::tr("Credentials required"))
            }
            Self::AuthFailed => write!(f, "{}", crate::i18n::tr("Authentication failed")),
            Self::HostUnreachable(msg) => {
                write!(f, "{}: {msg}", crate::i18n::tr("Host unreachable"))
            }
            Self::GvfsUnavailable(msg) => write!(f, "GVFS unavailable: {msg}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<glib::Error> for NetworkError {
    fn from(e: glib::Error) -> Self {
        let msg = e.message().to_owned();
        if e.kind::<gio::IOErrorEnum>() == Some(gio::IOErrorEnum::PermissionDenied)
            || msg.to_ascii_lowercase().contains("permission")
            || msg.to_ascii_lowercase().contains("access denied")
        {
            return Self::AuthFailed;
        }
        if e.kind::<gio::IOErrorEnum>() == Some(gio::IOErrorEnum::NetworkUnreachable)
            || e.kind::<gio::IOErrorEnum>() == Some(gio::IOErrorEnum::ConnectionRefused)
            || e.kind::<gio::IOErrorEnum>() == Some(gio::IOErrorEnum::TimedOut)
            || e.kind::<gio::IOErrorEnum>() == Some(gio::IOErrorEnum::HostNotFound)
        {
            return Self::HostUnreachable(msg);
        }
        if msg.to_ascii_lowercase().contains("gvfs")
            || msg.to_ascii_lowercase().contains("no such backend")
        {
            return Self::GvfsUnavailable(msg);
        }
        Self::Other(msg)
    }
}

#[derive(Debug, Clone, Default)]
pub struct NetworkCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
    pub domain: Option<String>,
    pub anonymous: bool,
}

impl NetworkCredentials {
    pub fn anonymous() -> Self {
        Self {
            anonymous: true,
            ..Default::default()
        }
    }

    pub fn with_password(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: Some(username.into()),
            password: Some(password.into()),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkEntry {
    pub display_name: String,
    pub uri: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: i64,
    pub icon_name: Option<String>,
    pub protocol: Option<NetworkProtocol>,
}

/// Reads the first themed icon name off a `GIcon`, or `None` when the icon
/// is not a `GThemedIcon`. Do not use `icon.to_string()` here: for a
/// `GThemedIcon` that yields the debug repr (`". GThemedIcon inode-directory …"`)
/// which is not a valid icon name and forces every caller to fall back to a
/// slow path in `get_icon_for_path`.
#[inline]
fn themed_icon_name(icon: Option<gio::Icon>) -> Option<String> {
    let icon = icon?;
    let themed = icon.downcast_ref::<gio::ThemedIcon>()?;
    themed.names().first().map(|s| s.to_string())
}

/// True when the error is `G_IO_ERROR_ALREADY_MOUNTED`. That is not a real
/// failure: the volume is attached and the subsequent `enumerate_children`
/// or `make_directory` call will proceed against the existing mount.
#[inline]
fn is_already_mounted(e: &glib::Error) -> bool {
    e.kind::<gio::IOErrorEnum>() == Some(gio::IOErrorEnum::AlreadyMounted)
        || e.matches(gio::IOErrorEnum::AlreadyMounted)
}

pub async fn list_network_entries(
    uri: &str,
    credentials: Option<&NetworkCredentials>,
) -> Result<Vec<NetworkEntry>, NetworkError> {
    net_debug!("[network] ────────────────────────────────────────────────");
    net_debug!("[network] list_network_entries ENTER uri = {uri:?}");
    net_debug!(
        "[network]   credentials = {:?}",
        credentials.map(|c| {
            format!(
                "user={:?} anon={} domain={:?}",
                c.username, c.anonymous, c.domain
            )
        })
    );

    let cancellable = gio::Cancellable::new();
    let file = gio::File::for_uri(uri);
    net_debug!("[network]   file.uri() = {:?}", file.uri());
    net_debug!("[network]   file.path() = {:?}", file.path());
    net_debug!("[network]   file.basename() = {:?}", file.basename());

    let mount_op = build_mount_op(credentials);
    net_debug!("[network]   calling mount_enclosing_volume…");
    match mount_enclosing_volume(&file, &mount_op).await {
        Ok(()) => net_debug!("[network]   mount_enclosing_volume OK"),
        Err(e) if is_already_mounted(&e) => {
            net_debug!("[network]   mount_enclosing_volume: already mounted");
        }
        Err(e) => eprintln!("[network]   mount_enclosing_volume FAILED: {e}"),
    }

    // WARNING: do not add `standard::content-type` here. GVFS's Google Drive
    // backend resolves it per entry with a full metadata fetch, which turns a
    // listing into an N-round-trip operation. `standard::type` is enough to
    // distinguish files from directories on every mainstream backend.
    let attributes =
        "standard::name,standard::display-name,standard::type,standard::size,time::modified,standard::icon";
    net_debug!("[network]   attributes = {attributes:?}");

    net_debug!("[network]   calling enumerate_children…");
    let enumerator = match file.enumerate_children(
        attributes,
        gio::FileQueryInfoFlags::NONE,
        Some(&cancellable),
    ) {
        Ok(e) => {
            net_debug!("[network]   enumerate_children OK");
            e
        }
        Err(e) => {
            eprintln!("[network]   enumerate_children FAILED: {e}");
            eprintln!("[network]   error kind: {:?}", e.kind::<gio::IOErrorEnum>());
            return Err(classify_enum_error(e, uri));
        }
    };

    let mut entries = Vec::new();
    let mut raw_count = 0usize;

    for info in enumerator.flatten() {
        raw_count += 1;
        let name = info.name();
        let file_type = info.file_type();
        let size = info.size();
        let has_icon = info.icon().is_some();

        net_debug!(
            "[network]   raw#{raw_count} name={name:?} type={file_type:?} size={size} \
             has_icon={has_icon}"
        );

        let display_name = info.display_name().to_string();
        let child_file = file.child(info.name());
        let child_uri = child_file.uri().to_string();

        let is_dir = match file_type {
            gio::FileType::Directory => true,
            gio::FileType::Regular => false,
            _ => info.name().to_string_lossy().ends_with('/'),
        };

        let size = info.size().max(0) as u64;
        let mtime = info
            .modification_date_time()
            .map(|dt| dt.to_unix())
            .unwrap_or(0);

        let mut icon_name = themed_icon_name(info.icon());

        if !is_dir {
            if let Some(ref name) = icon_name {
                if name.contains("folder") || name.contains("directory") || name.contains("server")
                {
                    icon_name = None;
                }
            }
        }

        let protocol = protocol_for_uri(&child_uri);

        net_debug!(
            "[network]     → push display_name={display_name:?} child_uri={child_uri:?} \
             is_dir={is_dir} size={size} mtime={mtime} icon_name={icon_name:?} proto={protocol:?}"
        );

        entries.push(NetworkEntry {
            display_name,
            uri: child_uri,
            is_dir,
            size,
            mtime,
            icon_name,
            protocol,
        });
    }

    net_debug!(
        "[network]   enumeration done: raw_count={raw_count} entries.len()={}",
        entries.len()
    );

    entries.sort_unstable_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
        })
    });

    net_debug!(
        "[network] list_network_entries EXIT uri = {uri:?} ({} entries)",
        entries.len()
    );
    net_debug!("[network] ────────────────────────────────────────────────");

    Ok(entries)
}

pub async fn create_network_directory(
    uri: &str,
    credentials: Option<&NetworkCredentials>,
) -> Result<(), NetworkError> {
    net_debug!("[network] create_network_directory uri = {uri:?}");
    let file = gio::File::for_uri(uri);
    let mount_op = build_mount_op(credentials);
    match mount_enclosing_volume(&file, &mount_op).await {
        Ok(()) => net_debug!("[network]   mount_enclosing_volume OK"),
        Err(e) if is_already_mounted(&e) => {
            net_debug!("[network]   mount_enclosing_volume: already mounted");
        }
        Err(e) => eprintln!("[network]   mount_enclosing_volume FAILED: {e}"),
    }
    match file.make_directory(None::<&gio::Cancellable>) {
        Ok(()) => {
            net_debug!("[network]   make_directory OK");
            Ok(())
        }
        Err(e) => {
            eprintln!("[network]   make_directory FAILED: {e}");
            Err(NetworkError::from(e))
        }
    }
}

pub fn entries_to_load_contexts(
    entries: &[NetworkEntry],
    expand_labels: bool,
) -> Vec<FileLoadContext> {
    net_debug!(
        "[network] entries_to_load_contexts: {} entries",
        entries.len()
    );
    entries
        .iter()
        .map(|e| {
            let target_path = PathBuf::from(&e.uri);
            let sort_name = e.display_name.to_lowercase();
            let sort_ext = std::path::Path::new(&e.display_name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            let custom_icon = if e.is_dir {
                e.icon_name
                    .clone()
                    .or_else(|| e.protocol.as_ref().map(|p| p.icon_name().to_owned()))
            } else {
                None
            };

            FileLoadContext::with_stats(
                e.display_name.clone(),
                target_path,
                e.is_dir,
                sort_name,
                sort_ext,
                e.size,
                e.mtime,
                None,
                expand_labels,
                custom_icon,
            )
        })
        .collect()
}

pub fn describe_network_location(uri: &str) -> (String, String) {
    net_debug!("[network] describe_network_location uri = {uri:?}");
    if uri.is_empty() || uri.contains('\0') {
        return (String::new(), "folder-remote-symbolic".to_owned());
    }

    let file = gio::File::for_uri(uri);
    let display = file
        .query_info(
            "standard::display-name,standard::icon",
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        )
        .ok();

    let name = display
        .as_ref()
        .map(|i| i.display_name().to_string())
        .unwrap_or_else(|| uri_display_name(uri));

    let icon = display
        .and_then(|i| themed_icon_name(i.icon()))
        .unwrap_or_else(|| {
            protocol_for_uri(uri)
                .map(|p| p.icon_name().to_owned())
                .unwrap_or_else(|| "folder-remote-symbolic".to_owned())
        });

    net_debug!("[network]   → name={name:?} icon={icon:?}");
    (name, icon)
}

pub async fn unmount_network_location(uri: &str) -> Result<(), NetworkError> {
    net_debug!("[network] unmount_network_location uri = {uri:?}");
    let file = gio::File::for_uri(uri);
    let mount = file
        .find_enclosing_mount(gio::Cancellable::NONE)
        .map_err(NetworkError::from)?;

    net_debug!("[network]   found mount: {:?}", mount.name());
    let mount_op = gio::MountOperation::new();
    match unmount_with_operation(&mount, &mount_op).await {
        Ok(()) => {
            net_debug!("[network]   unmount OK");
            Ok(())
        }
        Err(e) => {
            eprintln!("[network]   unmount FAILED: {e}");
            Err(NetworkError::from(e))
        }
    }
}

pub fn active_mounts() -> Vec<(String, String, String)> {
    let result: Vec<(String, String, String)> = gio::VolumeMonitor::get()
        .mounts()
        .into_iter()
        .filter_map(|mount| {
            let root = mount.root();
            let uri = root.uri().to_string();

            if !is_network_uri(&PathBuf::from(&uri)) {
                return None;
            }

            let name = mount.name().to_string();
            let icon = mount
                .icon()
                .downcast::<gio::ThemedIcon>()
                .ok()
                .and_then(|themed| themed.names().first().map(|s| s.to_string()))
                .unwrap_or_else(|| "folder-remote-symbolic".to_owned());

            Some((uri, name, icon))
        })
        .collect();
    net_debug!("[network] active_mounts → {} mounts", result.len());
    for (uri, name, _) in &result {
        net_debug!("[network]   mount: {name:?} → {uri:?}");
    }
    result
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct NetworkBookmark {
    pub name: String,
    pub uri: String,
    #[serde(default = "default_network_icon")]
    pub icon: String,
}

fn default_network_icon() -> String {
    "folder-remote-symbolic".to_owned()
}

impl NetworkBookmark {
    pub fn new(name: impl Into<String>, uri: impl Into<String>) -> Self {
        let uri = uri.into();
        let icon = protocol_for_uri(&uri)
            .map(|p| p.icon_name().to_owned())
            .unwrap_or_else(|| "folder-remote-symbolic".to_owned());
        Self {
            name: name.into(),
            uri,
            icon,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConnectToServerParams {
    pub protocol: String,
    pub host: String,
    pub port: Option<u16>,
    pub path: Option<String>,
    pub username: Option<String>,
}

impl ConnectToServerParams {
    pub fn build_uri(&self) -> Option<String> {
        if self.host.is_empty() {
            return None;
        }
        let scheme = match self.protocol.as_str() {
            "smb" => "smb",
            "sftp" | "ssh" => "sftp",
            "dav" => "dav",
            "davs" => "davs",
            "nfs" => "nfs",
            "ftp" => "ftp",
            "ftps" => "ftps",
            "afp" => "afp",
            "mtp" => "mtp",
            _ => return None,
        };

        let port_part = self.port.map(|p| format!(":{p}")).unwrap_or_default();

        let user_part = self
            .username
            .as_deref()
            .filter(|u| !u.is_empty())
            .map(|u| format!("{u}@"))
            .unwrap_or_default();

        let path_part = self
            .path
            .as_deref()
            .map(|p| {
                if p.starts_with('/') {
                    p.to_owned()
                } else {
                    format!("/{p}")
                }
            })
            .unwrap_or_else(|| "/".to_owned());

        let uri = format!(
            "{scheme}://{user_part}{host}{port_part}{path_part}",
            host = self.host,
        );
        net_debug!("[network] ConnectToServerParams::build_uri → {uri:?}");
        Some(uri)
    }
}

fn build_mount_op(credentials: Option<&NetworkCredentials>) -> gio::MountOperation {
    let op = gio::MountOperation::new();

    if let Some(creds) = credentials {
        if creds.anonymous {
            net_debug!("[network] build_mount_op: anonymous=true");
            op.set_anonymous(true);
            return op;
        }
        if let Some(ref user) = creds.username {
            op.set_username(Some(user.as_str()));
        }
        if let Some(ref pwd) = creds.password {
            op.set_password(Some(pwd.as_str()));
        }
        if let Some(ref domain) = creds.domain {
            op.set_domain(Some(domain.as_str()));
        }
        op.set_password_save(gio::PasswordSave::ForSession);
    }

    op
}

async fn mount_enclosing_volume(
    file: &gio::File,
    mount_op: &gio::MountOperation,
) -> Result<(), glib::Error> {
    file.mount_enclosing_volume_future(gio::MountMountFlags::NONE, Some(mount_op))
        .await
}

async fn unmount_with_operation(
    mount: &gio::Mount,
    mount_op: &gio::MountOperation,
) -> Result<(), glib::Error> {
    mount
        .unmount_with_operation_future(gio::MountUnmountFlags::NONE, Some(mount_op))
        .await
}

fn classify_enum_error(e: glib::Error, uri: &str) -> NetworkError {
    let msg = e.message().to_owned();
    eprintln!("[network] classify_enum_error uri={uri:?} msg={msg:?}");

    if e.kind::<gio::IOErrorEnum>() == Some(gio::IOErrorEnum::NotSupported)
        || msg.contains("Operation not supported")
    {
        return NetworkError::GvfsUnavailable(format!(
            "GVFS backend for '{uri}' not available. \
             Install gvfs and the relevant backend package (e.g. gvfs-smb, gvfs-fuse)."
        ));
    }

    if e.kind::<gio::IOErrorEnum>() == Some(gio::IOErrorEnum::PermissionDenied) {
        return NetworkError::CredentialsRequired {
            message: msg,
            flags: NetworkAuthFlags::USERNAME | NetworkAuthFlags::PASSWORD,
        };
    }

    NetworkError::from(e)
}

fn uri_display_name(uri: &str) -> String {
    let without_scheme = uri.split("://").nth(1).unwrap_or(uri).trim_end_matches('/');
    if without_scheme.is_empty() {
        return "".to_owned();
    }
    without_scheme.to_owned()
}

pub fn check_gvfs_deps() -> HashMap<&'static str, bool> {
    let mut results = HashMap::new();
    for binary in ["gvfsd", "gvfsd-smb", "gvfsd-sftp", "gvfsd-ftp", "gvfsd-nfs"] {
        let found = std::process::Command::new("which")
            .arg(binary)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        results.insert(binary, found);
    }
    net_debug!("[network] check_gvfs_deps → {results:?}");
    results
}
