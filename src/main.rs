//! Binary entry point. All application code lives in the `flux` library (`src/lib.rs`).

// The global allocator is set once, by the final binary, so it stays here.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    flux::cli::run();
}
