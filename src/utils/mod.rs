//! Core utility functions and cross-cutting concerns for Flux.
//!
//! This module provides shared logic for path resolution, filesystem
//! operations, and shell command execution.

// 1. Internal submodules
pub mod config;
pub mod deps;
pub mod extension_template;
pub mod glob;
pub mod helpers;
pub mod media;
pub mod path;
pub mod search;
pub mod xattr;

// 2. Public Re-exports
pub use config::*;
pub use path::{osstr_to_bytes, strip_current_dir, PathExt};

/// System-level configuration constants for utilities.
pub mod constants {}
