mod archive;
mod clipboard;
mod command;
mod dnd;
mod link;
mod rename;
mod system;
mod trash;

// Public so integration tests (and other crates) can reach the command builder.
#[allow(unused_imports)]
pub use command::{build_execution_command, should_track_in_transfer_dialog};
