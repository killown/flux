use gtk::gio;

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

pub(super) fn classify_enum_error(e: glib::Error, uri: &str) -> NetworkError {
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
