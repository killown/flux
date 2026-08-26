//! Core utility functions and cross-cutting concerns for Flux.
//!
//! This module provides shared logic for path resolution, filesystem
//! operations, and shell command execution.

// 1. Internal submodules
pub mod config;
pub mod deps;
pub mod glob;
pub mod helpers;
pub mod media;
pub mod path;
pub mod search;
pub mod xattr;

// 2. Public Re-exports
// This allows the rest of the app to use `utils::resolve` instead of `utils::path::resolve`.
pub use config::*;
pub use path::PathExt;

/// System-level configuration constants for utilities.
pub mod constants {}
