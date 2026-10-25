//! Archive password storage in the system keyring.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Value of the `application` attribute on every secret Flux stores.
pub const APPLICATION_ID: &str = "io.github.killown.flux";

/// Value of the `kind` attribute that marks an archive password.
pub const KIND_ARCHIVE_PASSWORD: &str = "archive-password";

/// Identity of one archive file at one point in time.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArchiveKey {
    /// Canonical (symlink-resolved, absolute) path of the archive.
    pub path: String,
    /// Modification time in whole seconds since the Unix epoch.
    pub mtime_secs: u64,
}

impl ArchiveKey {
    /// Builds a key from an already-canonical path string and an mtime.
    pub fn new(canonical_path: impl Into<String>, mtime_secs: u64) -> Self {
        Self {
            path: canonical_path.into(),
            mtime_secs,
        }
    }

    /// Resolves `archive` on disk and builds its key.
    pub async fn for_path(archive: &Path) -> std::io::Result<Self> {
        let canonical = tokio::fs::canonicalize(archive).await?;
        let modified = tokio::fs::metadata(&canonical).await?.modified()?;
        // A pre-1970 mtime clamps to 0 rather than failing the whole lookup.
        let secs = modified
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(Self::new(canonical.to_string_lossy(), secs))
    }

    /// The Secret Service attributes that identify this archive.
    pub fn attributes(&self) -> HashMap<&'static str, String> {
        HashMap::from([
            ("application", APPLICATION_ID.to_string()),
            ("kind", KIND_ARCHIVE_PASSWORD.to_string()),
            ("archive-path", self.path.clone()),
            ("archive-mtime", self.mtime_secs.to_string()),
        ])
    }

    /// Attributes that match this archive at **any** mtime.
    pub fn path_attributes(&self) -> HashMap<&'static str, String> {
        let mut attrs = self.attributes();
        attrs.remove("archive-mtime");
        attrs
    }

    /// Human-readable item label shown by keyring tools such as Seahorse.
    pub fn label(&self) -> String {
        let name = Path::new(&self.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.clone());
        format!("Flux archive password: {name}")
    }
}

/// Resolves the key for `archive`, logging and returning `None` on failure.
async fn resolve_key(archive: &Path, op: &str) -> Option<ArchiveKey> {
    match ArchiveKey::for_path(archive).await {
        Ok(key) => Some(key),
        Err(e) => {
            eprintln!("[keyring] {op}: cannot stat {}: {e}", archive.display());
            None
        }
    }
}

/// Returns the saved password for `archive`, if one exists and can be read.
pub async fn lookup_archive_password(archive: &Path) -> Option<String> {
    let key = resolve_key(archive, "lookup").await?;
    match try_lookup(&key).await {
        Ok(password) => password,
        Err(e) => {
            eprintln!("[keyring] lookup failed for {}: {e}", archive.display());
            None
        }
    }
}

async fn try_lookup(key: &ArchiveKey) -> Result<Option<String>, String> {
    let keyring = oo7::Keyring::new()
        .await
        .map_err(|e| format!("keyring unavailable: {e}"))?;
    let attributes = key.attributes();

    // Searching a locked collection is allowed and only returns locked
    // handles, so check for a match before asking the user to unlock anything.
    let items = keyring
        .search_items(&attributes)
        .await
        .map_err(|e| format!("search failed: {e}"))?;
    let Some(item) = items.first() else {
        return Ok(None);
    };

    keyring
        .unlock()
        .await
        .map_err(|e| format!("unlock failed: {e}"))?;
    let secret = item
        .secret()
        .await
        .map_err(|e| format!("read failed: {e}"))?;
    String::from_utf8(secret.as_bytes().to_vec())
        .map(Some)
        .map_err(|_| "stored secret is not valid UTF-8".to_string())
}

/// Saves `password` for `archive`, replacing any earlier secret for the same
/// archive and mtime. Returns `true` on success.
pub async fn save_archive_password(archive: &Path, password: &str) -> bool {
    let Some(key) = resolve_key(archive, "save").await else {
        return false;
    };
    match try_save(&key, password).await {
        Ok(()) => true,
        Err(e) => {
            eprintln!("[keyring] save failed for {}: {e}", archive.display());
            false
        }
    }
}

async fn try_save(key: &ArchiveKey, password: &str) -> Result<(), String> {
    let keyring = oo7::Keyring::new()
        .await
        .map_err(|e| format!("keyring unavailable: {e}"))?;
    keyring
        .unlock()
        .await
        .map_err(|e| format!("unlock failed: {e}"))?;
    if let Err(e) = keyring.delete(&key.path_attributes()).await {
        eprintln!("[keyring] stale-entry sweep failed for {}: {e}", key.path);
    }
    keyring
        .create_item(&key.label(), &key.attributes(), password, true)
        .await
        .map_err(|e| format!("write failed: {e}"))?;
    Ok(())
}

/// Deletes every saved password for `archive`, whatever mtime it was saved
/// under. Returns `true` when the keyring call succeeded, including when there
/// was nothing to delete.
pub async fn forget_archive_password(archive: &Path) -> bool {
    let Some(key) = resolve_key(archive, "forget").await else {
        return false;
    };
    match try_forget(&key).await {
        Ok(()) => true,
        Err(e) => {
            eprintln!("[keyring] forget failed for {}: {e}", archive.display());
            false
        }
    }
}

async fn try_forget(key: &ArchiveKey) -> Result<(), String> {
    let keyring = oo7::Keyring::new()
        .await
        .map_err(|e| format!("keyring unavailable: {e}"))?;
    keyring
        .unlock()
        .await
        .map_err(|e| format!("unlock failed: {e}"))?;
    keyring
        .delete(&key.path_attributes())
        .await
        .map_err(|e| format!("delete failed: {e}"))
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
