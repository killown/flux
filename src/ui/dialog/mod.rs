//! Dialog windows, prompts and choosers.
//!
//! Standalone dialogs (`command`, `conflict`, `network`, `transfer`) expose
//! their own constructors, the rest only add `show_*` methods to `FluxApp`.

mod about;
mod archive;
pub mod command;
pub mod conflict;
mod create;
mod icon;
mod inspector;
mod luks;
pub mod menu_icon_picker;
mod new_action;
mod open_with;
mod paste;
mod sidebar;
pub mod tag_picker;
pub mod template_editor;
pub mod transfer;
