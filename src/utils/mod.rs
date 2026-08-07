//! Core utility functions and cross-cutting concerns for Flux.
//!
//! This module provides shared logic for path resolution, filesystem
//! operations, and shell command execution.

// 1. Internal submodules
mod core;
pub mod deps;
pub(crate) mod helpers;
pub mod media;
mod path;
pub mod search;

// 2. Public Re-exports
// This allows the rest of the app to use `utils::resolve` instead of `utils::path::resolve`.
pub use core::*;
pub use path::PathExt;

/// System-level configuration constants for utilities.
pub mod constants {}
