//! Standalone single-file compressed archive backend.

use crate::services::archive::util::{lc_name, open_buf, strip_ext};
use crate::services::archive::{ArchiveBackend, ArchiveEntry, ArchiveError};
use std::io::Read;
use std::path::{Path, PathBuf};

pub(in crate::services::archive) struct SingleFileBackend; // for .gz, .bz2, .xz, .lzma, .zst, .lz4

impl ArchiveBackend for SingleFileBackend {
    fn list_entries(
        &self,
        archive_path: &Path,
        _prefix: &str,
        _password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        let name_lc = lc_name(archive_path);
        let inner_name = if name_lc.ends_with(".gz") {
            strip_ext(&name_lc, ".gz")
        } else if name_lc.ends_with(".bz2") {
            strip_ext(&name_lc, ".bz2")
        } else if name_lc.ends_with(".xz") {
            strip_ext(&name_lc, ".xz")
        } else if name_lc.ends_with(".lzma") {
            strip_ext(&name_lc, ".lzma")
        } else if name_lc.ends_with(".zstd") {
            strip_ext(&name_lc, ".zstd")
        } else if name_lc.ends_with(".zst") {
            strip_ext(&name_lc, ".zst")
        } else if name_lc.ends_with(".lz4") {
            strip_ext(&name_lc, ".lz4")
        } else {
            return Err(ArchiveError::Other("Unsupported single-file format".into()));
        };
        single_file_listing(archive_path, inner_name)
    }

    fn extract_entry_bytes(
        &self,
        archive_path: &Path,
        _inner_path: &str,
        _password: Option<&str>,
    ) -> Result<Vec<u8>, ArchiveError> {
        // Extract the whole file (the only entry) to bytes.
        let name_lc = lc_name(archive_path);
        let reader: Box<dyn Read> = if name_lc.ends_with(".gz") {
            Box::new(flate2::read::GzDecoder::new(open_buf(archive_path)?))
        } else if name_lc.ends_with(".bz2") {
            Box::new(bzip2::read::BzDecoder::new(open_buf(archive_path)?))
        } else if name_lc.ends_with(".xz") || name_lc.ends_with(".lzma") {
            Box::new(xz2::read::XzDecoder::new(open_buf(archive_path)?))
        } else if name_lc.ends_with(".zstd") || name_lc.ends_with(".zst") {
            let dec = zstd::stream::read::Decoder::new(open_buf(archive_path)?)
                .map_err(|e| ArchiveError::Other(format!("zstd: {e}")))?;
            Box::new(dec)
        } else if name_lc.ends_with(".lz4") {
            Box::new(lz4_flex::frame::FrameDecoder::new(open_buf(archive_path)?))
        } else {
            return Err(ArchiveError::Other("Unsupported single-file format".into()));
        };
        let mut data = Vec::new();
        let mut reader = reader;
        std::io::copy(&mut reader, &mut data)
            .map_err(|e| ArchiveError::Other(format!("decompress: {e}")))?;
        Ok(data)
    }

    fn extract_dir(
        &self,
        _archive_path: &Path,
        _inner_dir: &str,
        _password: Option<&str>,
    ) -> Result<PathBuf, ArchiveError> {
        Err(ArchiveError::Other(
            "Directory extraction from single-file archives is not supported".into(),
        ))
    }
}

// ─── Standalone compressed single-file archives ───────────────────────────────

/// Produces a listing with a single virtual entry - the decompressed filename.
/// Used for `.gz`, `.bz2`, `.xz`, `.zst`, `.lz4` files that are not tarballs.
fn single_file_listing(
    archive_path: &Path,
    inner_name: &str,
) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    if inner_name.is_empty() {
        return Ok(vec![]);
    }
    Ok(vec![ArchiveEntry {
        name: inner_name.to_owned(),
        is_dir: false,
        // Size is unknown without full decompression, 0 is acceptable for display.
        size: 0,
        mtime: archive_path
            .metadata()
            .ok()
            .and_then(|m| {
                m.modified().ok().and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_secs() as i64)
                })
            })
            .unwrap_or(0),
        inner_path: inner_name.to_owned(),
        is_encrypted: false,
        child_count: 0,
    }])
}
