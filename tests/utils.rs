mod utils {
    mod config_test;
    mod content_search_test;
    mod deps_test;
    mod glob_test;
    mod helpers_test;
    mod hwga_test;
    mod media_test;
    mod search_parser_test;
    mod thumbnail_tiers_test;
    mod xattr_test;

    use std::sync::Mutex;

    /// Single lock that serialises every test in this binary that mutates
    /// process-global env vars (`HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
    /// `XDG_CACHE_HOME`, `USER`) or reads code that depends on them.
    pub(super) static ENV_LOCK: Mutex<()> = Mutex::new(());
}
