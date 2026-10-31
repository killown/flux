//! Peek-preview panel handlers. Toggled by `Ctrl+Space` from the file grid.

mod compute;
mod handlers;
mod probe;
mod wide;

pub(crate) use compute::compute_preview;
