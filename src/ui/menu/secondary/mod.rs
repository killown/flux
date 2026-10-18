//! Secondary (MIME-driven) context menu: template lookup, parsing, gesture and `FluxApp` handlers.

mod gesture;
mod handlers;
mod matching;
mod mime;
mod template;

pub use gesture::setup_secondary_menu_gesture;
pub use matching::action_matches_mime;
pub use mime::split_mime;

// Part of the module's public API, but not referenced outside it yet.
pub use template::{
    append_secondary_action, category_mime_pattern, secondary_template_filename, NewSecondaryAction,
};
#[allow(unused_imports)]
pub use template::{parse_secondary_template, resolve_secondary_menu_template};
