pub mod archive;
pub mod db;
pub mod fonts;
pub mod git;
pub mod inspector;
pub mod keyring;
pub mod loader;
pub mod luks;
pub mod mounts;
pub mod network;
pub mod search;
pub mod tasks;
pub mod terminal;
pub mod thumbnails;
pub mod trash;
pub mod window_control;

pub mod constants {
    pub use crate::domain::model::MAX_CONTENT_SEARCH_RESULTS;
    /// Recursive filename search flushes a batch to the UI at this many results.
    pub const SEARCH_BATCH_SIZE: usize = 500;
    /// Recursive filename search flushes a non-empty batch after this many milliseconds.
    pub const SEARCH_FLUSH_INTERVAL_MS: u64 = 16;
    /// Content search reads files in blocks of this many bytes.
    pub const CONTENT_SEARCH_CHUNK_BYTES: usize = 256 * 1024;
    /// Content search skips a file with a NUL byte in its first this-many bytes.
    pub const CONTENT_SEARCH_BINARY_SNIFF_BYTES: usize = 1024;
}
