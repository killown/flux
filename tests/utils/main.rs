//! Tests for `flux::utils`.

use std::sync::Mutex;

mod config;
mod config_save;
mod content_search;
mod deps;
mod editor_command;
mod glob;
mod helpers;
mod hwga;
mod media;
mod search_parser;
mod thumbnail_tiers;
mod xattr;

/// Single lock that serialises every test in this binary that mutates
/// process-global env vars (`HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
/// `XDG_CACHE_HOME`, `USER`) or reads code that depends on them.
pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());
