//! Archive listing data and the password hand-off state.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Describes a single entry within an archive as seen from a specific directory level.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub is_dir: bool,
    /// Uncompressed size in bytes, `0` for synthesised directory nodes.
    pub size: u64,
    /// Last-modified unix timestamp, `0` when unavailable.
    pub mtime: i64,
    /// Full inner path relative to the archive root, used to build child URIs.
    pub inner_path: String,
    /// Number of direct children inside this directory at the listed level.
    /// Always `0` for file entries. Populated by `collect_entry`.
    pub child_count: u64,
    #[allow(dead_code)]
    pub is_encrypted: bool,
}

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

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

/// Passwords the user asked to save but that no archive has accepted yet.
#[derive(Default)]
pub struct PendingSaves(HashMap<PathBuf, String>);

impl PendingSaves {
    /// Queues `password` for `archive`, replacing any earlier request for it.
    pub fn queue(&mut self, archive: PathBuf, password: String) {
        self.0.insert(archive, password);
    }

    /// Resolves the request for `archive` once a load has finished.
    pub fn confirm(&mut self, archive: &Path, accepted: Option<&str>) -> Option<String> {
        let queued = self.0.remove(archive)?;
        (accepted == Some(queued.as_str())).then_some(queued)
    }

    /// Drops the request for `archive` without saving (the unlock failed).
    pub fn discard(&mut self, archive: &Path) {
        self.0.remove(archive);
    }
}

// Hand-written so a derived `Debug` on `FluxApp` can never print a password.
impl fmt::Debug for PendingSaves {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingSaves")
            .field("queued", &self.0.len())
            .finish()
    }
}
