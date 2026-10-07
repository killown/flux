//! Every sliding side panel lives here.
//!
//! Shared infrastructure: [`spec::PanelSpec`], [`resize::resizable_panel`],
//! [`header::panel_header`].
//!
//! Concrete panels: [`tag`], [`diff`], [`search`].

pub mod diff;
pub mod header;
pub mod resize;
pub mod search;
pub mod spec;
pub mod tag;

#[allow(unused_imports)]
pub use header::panel_header;
#[allow(unused_imports)]
pub use resize::resizable_panel;
#[allow(unused_imports)]
pub use spec::PanelSpec;

// Convenience re-exports of the panel builders.
pub use diff::{apply_diff_markup, build_diff_panel};
pub use search::build_search_panel;
pub use tag::build_tag_panel;
