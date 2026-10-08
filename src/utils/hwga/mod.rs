#![cfg_attr(not(debug_assertions), allow(dead_code))]

mod globals;
mod guard;
mod macros;
mod monitor;
mod probe;
mod stats;

#[allow(unused_imports)]
pub use globals::{count, reset_all, MAIN_THREAD_BUDGET};
pub use guard::CallGuard;
pub use monitor::CallMonitor;
pub use probe::Probe;
pub use stats::FunctionStats;
