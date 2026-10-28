//! Session-only "is this archive locked?" hint for grid cards.

use crate::model::FileLoadContext;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// `PK\x03\x04`, the signature of a ZIP local file header.
const ZIP_LOCAL_HEADER_SIG: [u8; 4] = *b"PK\x03\x04";

/// General purpose bit 0: the entry is encrypted (ZipCrypto and AES both set it).
const ZIP_FLAG_ENCRYPTED: u16 = 0x0001;

/// How the lock state of an archive can be learned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockSource {
    /// Readable from the file itself.
    ZipHeader,
    /// Only known once the session has seen a password prompt or an unlock.
    SessionOnly,
}

fn lock_source(path: &Path) -> Option<LockSource> {
    let ext = path.extension().and_then(OsStr::to_str)?;
    if ext.eq_ignore_ascii_case("zip") {
        Some(LockSource::ZipHeader)
    } else if ext.eq_ignore_ascii_case("7z") || ext.eq_ignore_ascii_case("rar") {
        Some(LockSource::SessionOnly)
    } else {
        None
    }
}

/// `PK\x05\x06`, end of central directory record.
const ZIP_EOCD_SIG: [u8; 4] = *b"PK\x05\x06";
/// `PK\x01\x02`, central directory file header.
const ZIP_CENTRAL_SIG: [u8; 4] = *b"PK\x01\x02";
/// Most bytes of the central directory read per archive (about 800 entries).
const ZIP_CENTRAL_SCAN_CAP: u64 = 64 * 1024;

/// True when any entry in the first part of the central directory is encrypted.
fn zip_central_directory_encrypted(file: &mut std::fs::File, len: u64) -> bool {
    crate::hit!("zip_central_directory_encrypted");
    let tail_len = len.min(22 + 65_535);
    let mut tail = vec![0u8; tail_len as usize];
    if file.seek(SeekFrom::Start(len - tail_len)).is_err() || file.read_exact(&mut tail).is_err() {
        return false;
    }
    let Some(eocd) = tail.windows(4).rposition(|w| w == ZIP_EOCD_SIG) else {
        return false;
    };
    let Some(rec) = tail.get(eocd..eocd + 22) else {
        return false;
    };
    let cd_size = u32::from_le_bytes([rec[12], rec[13], rec[14], rec[15]]) as u64;
    let cd_offset = u32::from_le_bytes([rec[16], rec[17], rec[18], rec[19]]) as u64;
    if cd_size == 0xFFFF_FFFF || cd_offset == 0xFFFF_FFFF || cd_offset + cd_size > len {
        return false;
    }
    let mut cd = vec![0u8; cd_size.min(ZIP_CENTRAL_SCAN_CAP) as usize];
    if file.seek(SeekFrom::Start(cd_offset)).is_err() || file.read_exact(&mut cd).is_err() {
        return false;
    }
    let mut pos = 0usize;
    while let Some(h) = cd.get(pos..pos + 46) {
        if h[..4] != ZIP_CENTRAL_SIG {
            return false;
        }
        if u16::from_le_bytes([h[8], h[9]]) & ZIP_FLAG_ENCRYPTED != 0 {
            return true;
        }
        let var = u16::from_le_bytes([h[28], h[29]]) as usize
            + u16::from_le_bytes([h[30], h[31]]) as usize
            + u16::from_le_bytes([h[32], h[33]]) as usize;
        pos += 46 + var;
    }
    false
}

/// Reads the encryption flag of a ZIP: first local header, then central directory.
pub fn zip_is_encrypted(path: &Path) -> bool {
    crate::hit!("zip_is_encrypted");
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(len) = file.metadata().map(|m| m.len()) else {
        return false;
    };
    if len < 22 {
        return false;
    }
    let mut header = [0u8; 8];
    if file.read_exact(&mut header).is_err() || header[..4] != ZIP_LOCAL_HEADER_SIG {
        return false;
    }
    u16::from_le_bytes([header[6], header[7]]) & ZIP_FLAG_ENCRYPTED != 0
        || zip_central_directory_encrypted(&mut file, len)
}

/// Paths only, never secrets: what the loader needs to stamp lock hints.
#[derive(Debug, Clone, Default)]
pub struct ArchiveLockSnapshot {
    /// Archives the session has seen demand a password.
    encrypted: HashSet<PathBuf>,
    /// Archives with a password in the session cache.
    unlocked: HashSet<PathBuf>,
}

impl ArchiveLockSnapshot {
    pub fn new(encrypted: HashSet<PathBuf>, unlocked: HashSet<PathBuf>) -> Self {
        Self {
            encrypted,
            unlocked,
        }
    }

    /// ZIP files whose lock state is not yet known to the session. The caller
    /// probes these off the load path with [`zip_is_encrypted`].
    pub fn zip_probe_candidates(&self, items: &[FileLoadContext]) -> Vec<PathBuf> {
        items
            .iter()
            .filter(|c| {
                !c.is_dir
                    && !c.is_encrypted_archive
                    && lock_source(&c.target_path) == Some(LockSource::ZipHeader)
                    && !self.encrypted.contains(c.target_path.as_path())
            })
            .map(|c| c.target_path.clone())
            .collect()
    }

    /// Fills `is_encrypted_archive` and `has_cached_password` on `ctx` from the
    /// session state only. Performs no file I/O.
    pub fn stamp(&self, ctx: &mut FileLoadContext) {
        ctx.is_encrypted_archive = false;
        ctx.has_cached_password = false;
        if ctx.is_dir {
            return;
        }
        let Some(source) = lock_source(&ctx.target_path) else {
            return;
        };
        let learned = self.encrypted.contains(ctx.target_path.as_path());
        ctx.is_encrypted_archive = match source {
            // No file I/O here. Reading a ZIP header is a remote range read on
            // FUSE/cloud mounts and would stall the folder load, so ZIP headers
            // are probed after the grid is shown (see `zip_probe_candidates`).
            LockSource::ZipHeader | LockSource::SessionOnly => learned,
        };
        ctx.has_cached_password = self.unlocked.contains(ctx.target_path.as_path());
    }
}
