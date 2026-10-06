//! Filesystem search: filename/extension walks, content scanning and the shared forbidden-path filter.

mod content;
mod extension;
mod forbidden;
pub mod indexer;
mod scan;
mod types;

pub use content::start_content_search;
pub use extension::{start_advanced_search, start_extension_search};
pub use types::{AdvancedSearchParams, ExtensionMatch};

// Part of the previous public API of `content_scan`, not referenced outside the module yet.
#[allow(unused_imports)]
pub use scan::{scan_file, Hit, DEFAULT_MAX_FILE_BYTES};
