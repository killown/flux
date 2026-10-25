//! Archive password and LUKS passphrase storage in the system keyring.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Value of the `application` attribute on every secret Flux stores.
pub const APPLICATION_ID: &str = "io.github.killown.flux";

/// Value of the `kind` attribute that marks an archive password.
pub const KIND_ARCHIVE_PASSWORD: &str = "archive-password";

/// Value of the `kind` attribute that marks a LUKS volume passphrase.
pub const KIND_LUKS_PASSPHRASE: &str = "luks-passphrase";

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

/// Identity of one LUKS container file.
///
/// Unlike [`ArchiveKey`] there is no mtime component: a mounted container is
/// written to constantly, so an mtime-bound entry would be orphaned after every
/// session. The passphrase belongs to the container, not to a file revision.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LuksKey {
    /// Canonical (symlink-resolved, absolute) path of the container image.
    pub path: String,
}

impl LuksKey {
    /// Builds a key from an already-canonical path string.
    pub fn new(canonical_path: impl Into<String>) -> Self {
        Self {
            path: canonical_path.into(),
        }
    }

    /// Resolves `image` on disk and builds its key.
    pub async fn for_path(image: &Path) -> std::io::Result<Self> {
        let canonical = tokio::fs::canonicalize(image).await?;
        Ok(Self::new(canonical.to_string_lossy()))
    }

    /// The Secret Service attributes that identify this container.
    pub fn attributes(&self) -> HashMap<&'static str, String> {
        HashMap::from([
            ("application", APPLICATION_ID.to_string()),
            ("kind", KIND_LUKS_PASSPHRASE.to_string()),
            ("luks-path", self.path.clone()),
        ])
    }

    /// Human-readable item label shown by keyring tools such as Seahorse.
    pub fn label(&self) -> String {
        let name = Path::new(&self.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.clone());
        format!("Flux LUKS passphrase: {name}")
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
    lookup_secret(&key.attributes()).await
}

/// Reads the secret stored under exactly `attributes`, if any.
async fn lookup_secret(
    attributes: &HashMap<&'static str, String>,
) -> Result<Option<String>, String> {
    let keyring = oo7::Keyring::new()
        .await
        .map_err(|e| format!("keyring unavailable: {e}"))?;

    // Searching a locked collection is allowed and only returns locked
    // handles, so check for a match before asking the user to unlock anything.
    let items = keyring
        .search_items(attributes)
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
    store_secret(
        &key.path,
        &key.label(),
        &key.attributes(),
        &key.path_attributes(),
        password,
    )
    .await
}

/// Stores `secret` under `attributes`, first sweeping entries matching `stale`.
async fn store_secret(
    subject: &str,
    label: &str,
    attributes: &HashMap<&'static str, String>,
    stale: &HashMap<&'static str, String>,
    secret: &str,
) -> Result<(), String> {
    let keyring = oo7::Keyring::new()
        .await
        .map_err(|e| format!("keyring unavailable: {e}"))?;
    keyring
        .unlock()
        .await
        .map_err(|e| format!("unlock failed: {e}"))?;
    if let Err(e) = keyring.delete(stale).await {
        eprintln!("[keyring] stale-entry sweep failed for {subject}: {e}");
    }
    keyring
        .create_item(label, attributes, secret, true)
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
    delete_secrets(&key.path_attributes()).await
}

/// Deletes every secret whose attributes match `attributes`.
async fn delete_secrets(attributes: &HashMap<&'static str, String>) -> Result<(), String> {
    let keyring = oo7::Keyring::new()
        .await
        .map_err(|e| format!("keyring unavailable: {e}"))?;
    keyring
        .unlock()
        .await
        .map_err(|e| format!("unlock failed: {e}"))?;
    keyring
        .delete(attributes)
        .await
        .map_err(|e| format!("delete failed: {e}"))
}

/// Resolves the key for `image`, logging and returning `None` on failure.
async fn resolve_luks_key(image: &Path, op: &str) -> Option<LuksKey> {
    match LuksKey::for_path(image).await {
        Ok(key) => Some(key),
        Err(e) => {
            eprintln!("[keyring] {op}: cannot stat {}: {e}", image.display());
            None
        }
    }
}

/// Returns the saved passphrase for the LUKS container `image`, if one exists
/// and can be read.
pub async fn lookup_luks_passphrase(image: &Path) -> Option<String> {
    let key = resolve_luks_key(image, "lookup").await?;
    match lookup_secret(&key.attributes()).await {
        Ok(passphrase) => passphrase,
        Err(e) => {
            eprintln!("[keyring] lookup failed for {}: {e}", image.display());
            None
        }
    }
}

/// Saves `passphrase` for the LUKS container `image`, replacing any earlier
/// secret for the same container. Returns `true` on success.
pub async fn save_luks_passphrase(image: &Path, passphrase: &str) -> bool {
    let Some(key) = resolve_luks_key(image, "save").await else {
        return false;
    };
    let attributes = key.attributes();
    match store_secret(
        &key.path,
        &key.label(),
        &attributes,
        &attributes,
        passphrase,
    )
    .await
    {
        Ok(()) => true,
        Err(e) => {
            eprintln!("[keyring] save failed for {}: {e}", image.display());
            false
        }
    }
}

/// Deletes the saved passphrase for the LUKS container `image`. Returns `true`
/// when the keyring call succeeded, including when there was nothing to delete.
pub async fn forget_luks_passphrase(image: &Path) -> bool {
    let Some(key) = resolve_luks_key(image, "forget").await else {
        return false;
    };
    match delete_secrets(&key.attributes()).await {
        Ok(()) => true,
        Err(e) => {
            eprintln!("[keyring] forget failed for {}: {e}", image.display());
            false
        }
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
