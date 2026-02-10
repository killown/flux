//! Background services and asynchronous workers for Flux.
//!
//! This module contains infrastructure-level services that handle
//! long-running or resource-intensive tasks such as thumbnail
//! generation and metadata extraction.

// 1. Declare the background service modules
pub mod thumbnails;

/// Service-level constants, such as cache limits or thread counts.
pub mod constants {
    /// Maximum number of concurrent thumbnail generation tasks.
    pub const MAX_THUMBNAIL_THREADS: usize = 4;
}
