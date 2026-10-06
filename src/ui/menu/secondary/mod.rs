//! Secondary (MIME-driven) context menu: template lookup, parsing, gesture and `FluxApp` handlers.

mod gesture;
mod handlers;
mod mime;
mod template;

pub use gesture::setup_secondary_menu_gesture;

// Part of the module's public API, but not referenced outside it yet.
#[allow(unused_imports)]
pub use template::{parse_secondary_template, resolve_secondary_menu_template};
