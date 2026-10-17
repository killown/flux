// `AppMsg` carries a large `PreviewReady` variant. Every
// message router in `src/ui/update/` returns `Result<(), AppMsg>` as part
// of the chain-of-responsibility dispatch in `update/mod.rs`. Boxing the
// error at each of the dozen call sites would be churn for no runtime
// benefit (the `Ok` path never allocates), and boxing the variant inside
// `AppMsg` would touch the entire preview pipeline. Silenced here instead.
#![allow(clippy::result_large_err)]

pub mod args;
pub use crate::utils::hwga;
pub mod i18n;
pub mod model;
pub mod services;
pub mod ui;
pub mod utils;
