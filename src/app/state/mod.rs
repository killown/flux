//! Application state that holds toolkit handles and service objects.
//!
//! These types used to live in `domain::model`. They moved here because they own
//! widgets, GIO handles and infrastructure services, none of which `domain` may name.

mod flux_app;
mod panels;
mod session;
mod types;

pub use flux_app::FluxApp;
pub use panels::Panels;
pub use session::{PreviewState, SearchState, TerminalState};
pub use types::{CachedFolder, SENDER};
