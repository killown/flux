mod cli;
mod i18n;
mod model;
mod services;
mod ui;
mod utils;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    cli::run();
}
