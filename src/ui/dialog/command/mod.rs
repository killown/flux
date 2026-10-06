//! Command output window (`CommandDialogHandle`) plus the `FluxApp` handlers that drive it.

mod handlers;
mod procfs;
mod window;

pub use window::{create_command_dialog, CommandDialogHandle};
