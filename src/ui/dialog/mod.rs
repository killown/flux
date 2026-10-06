//! Dialog windows, prompts and choosers.
//!
//! Standalone dialogs (`command`, `conflict`, `network`, `transfer`) expose
//! their own constructors, the rest only add `show_*` methods to `FluxApp`.

pub mod command;
pub mod conflict;
pub mod network;
pub mod transfer;

mod about;
mod archive;
mod create;
mod icon;
mod inspector;
mod location;
mod luks;
mod open_with;
mod paste;
mod sidebar;
