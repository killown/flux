use std::sync::OnceLock;

/// Set `FLUX_DEBUG_NETWORK=1` to enable per-entry tracing on stderr.
/// Off by default so a slow mount doesn't drown the journal in
/// thousands of debug lines on every navigation.
pub(crate) fn net_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("FLUX_DEBUG_NETWORK").is_some())
}

macro_rules! net_debug {
    ($($arg:tt)*) => {
        if $crate::services::network::debug::net_debug_enabled() {
            eprintln!($($arg)*);
        }
    };
}

pub(crate) use net_debug;
