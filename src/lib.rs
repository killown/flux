// `AppMsg` carries a large `PreviewReady` variant. Every
// message router in `src/app/update/` returns `Result<(), AppMsg>` as part
// of the chain-of-responsibility dispatch in `update/mod.rs`. Boxing the
// error at each of the dozen call sites would be churn for no runtime
// benefit (the `Ok` path never allocates), and boxing the variant inside
// `AppMsg` would touch the entire preview pipeline. Silenced here instead.
#![allow(clippy::result_large_err)]

pub mod app;
pub mod args;
mod doc_cli;
mod doc_fish;
pub mod domain;
pub mod infra;
pub mod ipc;
pub use crate::utils::hwga;
pub mod i18n;
pub mod model;
pub mod services;
pub mod shell_functions;
pub mod ui;
pub mod utils;

/// Re-exports used only by `src/bin/gen-docs.rs`.
#[doc(hidden)]
pub mod _doc_gen {
    pub mod cli {
        pub use crate::doc_cli::render_cli_markdown;
    }
    pub mod config {
        pub use crate::model::render_config_markdown;
    }
    pub mod css_variables {
        pub use crate::ui::render_css_variables_markdown;
    }
    pub mod actions {
        pub use crate::ui::menu::editor::render_actions_markdown;
    }
    pub mod search {
        pub use crate::utils::search::render_search_markdown;
    }
    pub mod fish {
        pub use crate::doc_fish::{render_completions, render_integration};
    }
}
