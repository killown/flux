//! The `ArchiveBackend` trait and the backend factory.

use crate::services::archive::backends::{
    DebBackend, IsoBackend, RarBackend, SevenZBackend, SingleFileBackend, TarBackend,
    UnsupportedBackend, ZipBackend,
};
use crate::services::archive::util::lc_name;
use crate::services::archive::{ArchiveEntry, ArchiveError};
use std::path::{Path, PathBuf};

// ─── ArchiveBackend trait ────────────────────────────────────────────────────

/// Common interface for all archive format backends.
pub trait ArchiveBackend: Send + Sync {
    /// List entries at `prefix` inside the archive.
    fn list_entries(
        &self,
        archive_path: &Path,
        prefix: &str,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError>;

    /// Extract a single entry as raw bytes.
    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        inner_path: &str,
        password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError>;

    /// Extract a directory subtree to a temporary directory and return its root path.
    fn extract_dir(
        &self,
        archive_path: &Path,
        inner_dir: &str,
        password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError>;

    /// Deletes an inner path entry from the archive on disk.
    fn remove_entry(
        &self,
        _archive_path: &Path,
        _inner_path: &str,
        _password: Option<&str>,
        _progress_cb: Option<&mut dyn FnMut(u64)>,
    ) -> Result<(), ArchiveError> {
        Err(ArchiveError::Other(
            "In-archive deletion is not supported for this format".into(),
        ))
    }

    /// Adds or replaces an entry inside the archive from a local file on disk.
    fn write_entry(
        &self,
        _archive_path: &Path,
        _source_file: &Path,
        _inner_path: &str,
        _password: Option<&str>,
        _progress_cb: Option<&mut dyn FnMut(u64)>,
    ) -> Result<(), ArchiveError> {
        Err(ArchiveError::Other(
            "In-archive write is not supported for this format".into(),
        ))
    }

    /// Creates an empty directory entry inside the archive.
    fn create_directory(
        &self,
        _archive_path: &Path,
        _inner_path: &str,
        _password: Option<&str>,
    ) -> Result<(), ArchiveError> {
        Err(ArchiveError::Other(
            "In-archive directory creation is not supported for this format".into(),
        ))
    }
}

// ─── Factory ─────────────────────────────────────────────────────────────────

pub fn get_backend(
    archive_path: &Path,
    backend: Option<Box<dyn ArchiveBackend>>,
) -> Box<dyn ArchiveBackend> {
    if let Some(b) = backend {
        return b;
    }
    let name_lc = lc_name(archive_path);

    if name_lc.ends_with(".zip") {
        return Box::new(ZipBackend);
    } else if name_lc.ends_with(".7z") {
        return Box::new(SevenZBackend);
    } else if name_lc.ends_with(".tar.gz")
        || name_lc.ends_with(".tgz")
        || name_lc.ends_with(".tar.bz2")
        || name_lc.ends_with(".tbz2")
        || name_lc.ends_with(".tar.xz")
        || name_lc.ends_with(".txz")
        || name_lc.ends_with(".tar.lzma")
        || name_lc.ends_with(".tlz")
        || name_lc.ends_with(".tar.zst")
        || name_lc.ends_with(".tzst")
        || name_lc.ends_with(".tar.lz4")
        || name_lc.ends_with(".tar")
    {
        return Box::new(TarBackend);
    } else if name_lc.ends_with(".rar") {
        return Box::new(RarBackend);
    } else if name_lc.ends_with(".iso") {
        return Box::new(IsoBackend);
    } else if name_lc.ends_with(".deb") {
        return Box::new(DebBackend);
    }

    let mime = crate::utils::media::get_mime_type(archive_path);
    match mime.as_str() {
        "application/zip" | "application/x-zip-compressed" => Box::new(ZipBackend),
        "application/x-7z-compressed" => Box::new(SevenZBackend),
        "application/x-tar" => Box::new(TarBackend),
        "application/x-rar" | "application/x-rar-compressed" => Box::new(RarBackend),
        "application/gzip" | "application/x-bzip2" | "application/x-xz" | "application/zstd" => {
            Box::new(SingleFileBackend)
        }
        _ => Box::new(UnsupportedBackend),
    }
}
