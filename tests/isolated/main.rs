//! Tests that mutate process-global state (environment variables, HWGA probe
//! counters, the IPC socket path) and so must not share a process with the
//! other test binaries. Within this binary they take `ENV_LOCK` in turn.

use std::sync::Mutex;

/// Serialises every test here that sets `HOME` / `XDG_*` or resets probes.
pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());

mod hwga_regression;
mod ipc;
