use std::fs::Metadata;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Internal metadata container for parallel directory processing.
#[derive(Debug, Clone)]
pub struct FileLoadContext {
    /// The sanitized name used for UI rendering and label display.
    pub display_name: String,
    /// The absolute filesystem path to the target file or directory.
    pub target_path: PathBuf,
    /// True if the item is a directory, false if it is a file or symlink.
    pub is_dir: bool,
    /// Pre-processed string used for case-insensitive and natural sorting.
    pub sort_name: String,
    /// Pre-computed lowercase file extension for zero-alloc Type sorting.
    pub sort_ext: String,
    /// The path to a cached or generated image representing the file content.
    pub thumbnail_path: Option<PathBuf>,
    /// Whether the filename label should wrap across multiple lines.
    pub expand_labels: bool,
    /// Optional override icon name for the item.
    pub custom_icon: Option<String>,

    /// True if the target item is a symbolic link.
    pub is_symlink: bool,
    /// True if the symbolic link points to a non-existent target.
    pub is_broken_symlink: bool,

    // Lazy metadata caches
    /// Cached emptiness flag determined lazily without consuming entire directory streams.
    is_empty: OnceLock<bool>,
    metadata: OnceLock<Option<Metadata>>,
    size: OnceLock<u64>,
    mtime: OnceLock<i64>,
    is_foreign_owner: OnceLock<bool>,
}

impl FileLoadContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        display_name: String,
        target_path: PathBuf,
        is_dir: bool,
        sort_name: String,
        sort_ext: String,
        thumbnail_path: Option<PathBuf>,
        expand_labels: bool,
        custom_icon: Option<String>,
        is_symlink: bool,
        is_broken_symlink: bool,
    ) -> Self {
        Self {
            display_name,
            target_path,
            is_dir,
            sort_name,
            sort_ext,
            thumbnail_path,
            expand_labels,
            custom_icon,
            is_symlink,
            is_broken_symlink,
            is_empty: OnceLock::new(),
            metadata: OnceLock::new(),
            size: OnceLock::new(),
            mtime: OnceLock::new(),
            is_foreign_owner: OnceLock::new(),
        }
    }

    /// Resolves and caches file metadata on demand.
    #[inline]
    pub fn metadata(&self) -> Option<&Metadata> {
        self.metadata
            .get_or_init(|| {
                if self.is_symlink {
                    std::fs::metadata(&self.target_path)
                        .ok()
                        .or_else(|| self.target_path.symlink_metadata().ok())
                } else {
                    self.target_path.symlink_metadata().ok()
                }
            })
            .as_ref()
    }

    /// Pre-seeds size, modification time, and ownership for virtual/archive entries.
    #[allow(clippy::too_many_arguments)]
    pub fn with_stats(
        display_name: String,
        target_path: PathBuf,
        is_dir: bool,
        sort_name: String,
        sort_ext: String,
        size: u64,
        mtime: i64,
        thumbnail_path: Option<PathBuf>,
        expand_labels: bool,
        custom_icon: Option<String>,
    ) -> Self {
        Self {
            display_name,
            target_path,
            is_dir,
            sort_name,
            sort_ext,
            thumbnail_path,
            expand_labels,
            custom_icon,
            is_symlink: false,
            is_broken_symlink: false,
            is_empty: OnceLock::from(size == 0),
            metadata: OnceLock::new(),
            size: OnceLock::from(size),
            mtime: OnceLock::from(mtime),
            is_foreign_owner: OnceLock::from(false),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        *self.is_empty.get_or_init(|| {
            if self.is_dir {
                std::fs::read_dir(&self.target_path)
                    .map(|mut rd| rd.next().is_none())
                    .unwrap_or(false)
            } else {
                self.size() == 0
            }
        })
    }

    /// Resolves size lazily on first access.
    #[inline]
    pub fn size(&self) -> u64 {
        *self.size.get_or_init(|| {
            if self.is_dir {
                std::fs::read_dir(&self.target_path)
                    .map(|rd| rd.count())
                    .unwrap_or(0) as u64
            } else {
                self.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
    }

    /// Resolves modification time lazily on first access.
    #[inline]
    pub fn mtime(&self) -> i64 {
        *self.mtime.get_or_init(|| {
            self.metadata()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        })
    }

    /// Resolves foreign owner flag lazily on first access.
    #[inline]
    pub fn is_foreign_owner(&self, current_uid: u32) -> bool {
        *self.is_foreign_owner.get_or_init(|| {
            use std::os::unix::fs::MetadataExt;
            self.metadata()
                .map(|m| m.uid() != current_uid)
                .unwrap_or(false)
        })
    }
}
