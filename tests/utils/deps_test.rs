use flux::utils::deps::check_optional_deps;

#[test]
fn test_check_optional_deps_runs_without_panic() {
    // This executes the runtime availability checks against the host environment
    // to ensure they complete safely without panicking.
    check_optional_deps();
}
