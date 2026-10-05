//! Error type returned by archive listing and extraction.

// ─── Error type ───────────────────────────────────────────────────────────────

/// Error returned by listing/extraction operations.
#[derive(Debug, Clone)]
pub enum ArchiveError {
    /// The archive is encrypted and requires a password.
    PasswordRequired,
    /// The supplied password was rejected.
    WrongPassword,
    /// Any other I/O or format error, contains a human-readable message.
    Other(String),
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::PasswordRequired => {
                write!(f, "{}", crate::i18n::tr("Archive is password-protected"))
            }
            ArchiveError::WrongPassword => write!(f, "{}", crate::i18n::tr("Incorrect password")),
            ArchiveError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<String> for ArchiveError {
    fn from(s: String) -> Self {
        ArchiveError::Other(s)
    }
}
