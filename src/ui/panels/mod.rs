//! Every sliding side panel lives here.
//!
//! Shared infrastructure: [`spec::PanelSpec`], [`resize::resizable_panel`],
//! [`header::panel_header`].
//!
//! Concrete panels: [`tag`], [`diff`], [`search`].

pub mod diff;
pub mod header;
pub mod preview;
pub mod preview_binary;
pub mod preview_gutter;
pub mod preview_markdown;
pub mod preview_office;
pub mod preview_syntax;
pub mod preview_table;
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
pub(crate) mod preview_registry;

pub use diff::{apply_diff_markup, build_diff_panel};
pub use preview::build_preview_panel;
pub use search::build_search_panel;
pub use tag::build_tag_panel;
pub mod location;
pub use location::build_location_panel;
pub mod connect;
pub use connect::build_connect_panel;
