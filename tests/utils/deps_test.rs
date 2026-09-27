use flux::utils::deps::check_optional_deps;

#[test]
fn test_check_optional_deps_runs_without_panic() {
    // This executes the runtime availability checks against the host environment
    // to ensure they complete safely without panicking.
    check_optional_deps();
}

#[test]
fn check_optional_deps_is_idempotent() {
    // Must not leak state on repeated invocation.
    flux::utils::deps::check_optional_deps();
    flux::utils::deps::check_optional_deps();
}

#[test]
fn check_gvfs_deps_runs() {
    let results = flux::services::network::check_gvfs_deps();
    assert!(results.contains_key("gvfsd"));
}
