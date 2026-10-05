//! Fallback backend for unrecognised formats.

use crate::services::archive::{ArchiveBackend, ArchiveEntry, ArchiveError};
use std::path::{Path, PathBuf};

pub(in crate::services::archive) struct UnsupportedBackend;

impl ArchiveBackend for UnsupportedBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        _prefix: &str,
        _password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        Err(ArchiveError::Other(format!(
            "Unsupported format: {}",
            archive_path.display()
        )))
    }

    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        _inner_path: &str,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        Err(ArchiveError::Other(format!(
            "Unsupported format: {}",
            archive_path.display()
        )))
    }

    fn extract_dir(
        &self,
        archive_path: &Path,
        _inner_dir: &str,
        _password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        Err(ArchiveError::Other(format!(
            "Unsupported format: {}",
            archive_path.display()
        )))
    }
}
