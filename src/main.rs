// `AppMsg` carries a large `PreviewReady` variant. Every
// message router in `src/app/update/` returns `Result<(), AppMsg>` as part
// of the chain-of-responsibility dispatch in `update/mod.rs`. Boxing the
// error at each of the dozen call sites would be churn for no runtime
// benefit (the `Ok` path never allocates), and boxing the variant inside
// `AppMsg` would touch the entire preview pipeline. Silenced here as well
// because the binary crate is a separate compilation unit from `lib.rs`
// and does not inherit its `#![allow]`.
#![allow(clippy::result_large_err)]

mod app;
mod cli;
mod domain;
mod i18n;
mod infra;
mod ipc;
mod model;
mod shell_functions;
mod ui;
mod utils;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    cli::run();
}
