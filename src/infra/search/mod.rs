//! Filesystem search support: the file indexer, per-file content scanning and the shared
//! forbidden-path filter. The recursive filename and content walks live in `src/app/update/search/`.

mod forbidden;
pub mod indexer;
mod scan;

pub(crate) use forbidden::forbidden_matcher;
pub use scan::scan_file;

// Part of the previous public API of `content_scan`, not referenced outside the module yet.
#[allow(unused_imports)]
pub use scan::{Hit, DEFAULT_MAX_FILE_BYTES};
