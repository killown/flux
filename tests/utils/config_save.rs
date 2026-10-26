//! `save_config` must create its directory and persist what it was given.

use super::ENV_LOCK;
use flux::utils::config::{invalidate_config_cache, load_config, save_config};

#[test]
fn save_config_creates_the_missing_directory_and_round_trips() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sandbox = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", sandbox.path());
    std::env::set_var("XDG_CONFIG_HOME", sandbox.path().join("missing"));

    let mut config = flux::model::Config::default();
    config.ui.theme = Some("roundtrip-theme".to_string());
    save_config(&config);

    let path = sandbox.path().join("missing/flux/config.toml");
    let written = std::fs::read_to_string(&path).expect("config.toml must be written");
    assert!(written.contains("roundtrip-theme"));

    invalidate_config_cache();
    assert_eq!(load_config().ui.theme.as_deref(), Some("roundtrip-theme"));
}
