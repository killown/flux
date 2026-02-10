//! Core utility functions and cross-cutting concerns for Flux.
//!
//! This module provides shared logic for path resolution, filesystem
//! operations, and shell command execution.

// 1. Internal submodules
mod core;
mod helpers;
mod path;

// 2. Public Re-exports
// This allows the rest of the app to use `utils::resolve` instead of `utils::path::resolve`.
pub use core::*;
pub use helpers::*;
pub use path::{resolve, PathExt};

/// System-level configuration constants for utilities.
pub mod constants {
    /// The default fallback path if a config directory cannot be resolved.
    pub const FALLBACK_CONFIG_PATH: &str = "/tmp/flux";

    /// Placeholder used in custom menu commands for path interpolation.
    pub const PATH_PLACEHOLDER: &str = "%p";
}
