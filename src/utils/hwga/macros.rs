//! The `hit!` macro. Exported at the crate root via `#[macro_export]`.

/// Inserts a lock-free telemetry probe into the current scope.
///
/// - `hit!()` uses `module_path!()::line!()` automatically.
/// - `hit!("custom_name")` uses a stable name that won't shift if lines above it move.
///
/// Both compile down to absolute no-ops in release builds.
#[macro_export]
macro_rules! hit {
    () => {
        $crate::hit!(@probe concat!(module_path!(), "::", line!()));
    };
    ($name:literal) => {
        $crate::hit!(@probe concat!(module_path!(), "::", $name));
    };
    (@probe $full:expr) => {
        #[cfg(debug_assertions)]
        let _hwga_guard = {
            static PROBE: $crate::utils::hwga::Probe =
                $crate::utils::hwga::Probe::new($full);
            $crate::utils::hwga::CallGuard::new(&PROBE)
        };
    };
}
