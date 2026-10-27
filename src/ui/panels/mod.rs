//! Every sliding side panel lives here.
//!
//! Shared infrastructure: [`spec::PanelSpec`], [`resize::resizable_panel`],
//! [`header::panel_header`].
//!
//! Concrete panels: [`tag`], [`diff`], [`search`].

pub mod actions;
pub mod diff;
pub mod header;
pub mod preview;
pub mod preview_binary;
pub(crate) mod preview_folder_thumbs;
pub mod preview_gutter;
pub mod preview_markdown;
pub mod preview_office;
pub mod preview_syntax;
pub mod preview_table;
pub mod quick_list;
pub mod resize;
pub mod search;
pub mod spec;
pub mod tag;
pub mod terminal;

#[allow(unused_imports)]
pub use header::panel_header;
#[allow(unused_imports)]
pub use resize::resizable_panel;
#[allow(unused_imports)]
pub use spec::PanelSpec;
pub use terminal::build_terminal_panel;
pub(crate) mod actions_registry;
pub(crate) mod preview_registry;

pub use actions::build_actions_panel;
pub use diff::{apply_diff_markup, build_diff_panel};
pub use preview::build_preview_panel;
pub use quick_list::build_quick_list_panel;
pub use search::build_search_panel;
pub use tag::build_tag_panel;
pub mod location;
pub use location::build_location_panel;
pub mod connect;
pub use connect::build_connect_panel;
mod properties;
pub use properties::build_properties_panel;
