//! Background services and asynchronous workers for Flux.
//!
//! This module contains infrastructure-level services that handle
//! long-running or resource-intensive tasks such as thumbnail
//! generation and metadata extraction.

// 1. Declare the background service modules
pub mod archive;
pub mod content_search;
pub mod db;
pub mod extension_search;
pub mod loader;
pub mod luks;
pub mod network;
pub mod tasks;
pub mod terminal;
pub mod thumbnails;
pub mod trash;

/// Service-level constants, such as cache limits or thread counts.
pub mod constants {
    /// max content search results to return from the content search service
    pub const MAX_CONTENT_SEARCH_RESULTS: usize = 100;
}
