/// A single matched file, carried inside [`AppMsg::ExtensionSearchBatch`].
#[derive(Debug, Clone)]
pub struct ExtensionMatch {
    pub path: std::path::PathBuf,
    /// Display string shown in the result row (relative path from search root).
    pub display: String,
    pub size: u64,
    pub mtime: i64,
}

/// Optional constraints from the Advanced Search dialog.
///
/// When `None` fields are present the corresponding predicate is skipped,
/// so a default `AdvancedSearchParams` behaves identically to the plain
/// `start_extension_search` path.
#[derive(Debug, Clone, Default)]
pub struct AdvancedSearchParams {
    /// Glob patterns (already expanded from MIME shorthands) or regex rules.
    pub patterns: Vec<String>,
    /// Exclude files whose mtime is older than `now - date_seconds`.
    pub date_seconds: Option<u64>,
    /// `(larger_than, threshold_bytes)`:
    /// * `true`  → keep only files *larger than* the threshold
    /// * `false` → keep only files *smaller than* the threshold
    pub size_bytes: Option<(bool, u64)>,
    /// When `true`, dotfiles are included even if the global toggle is off.
    pub include_hidden: bool,
    /// When `true`, only directory entries match the search.
    pub only_folders: bool,
    /// Maximum matches allowed during the search walk before stopping.
    pub max_results: usize,
}
