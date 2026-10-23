//! The `LuksImage` newtype: a path to a LUKS container file on disk.

use std::path::PathBuf;

/// Represents a LUKS image file selected for mounting.
#[derive(Debug, Clone)]
pub struct LuksImage {
    pub path: PathBuf,
}
