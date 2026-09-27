#![no_main]
use libfuzzer_sys::fuzz_target;
use std::sync::Once;

static INIT_FUZZ_ENV: Once = Once::new();

fuzz_target!(|data: &[u8]| {
    INIT_FUZZ_ENV.call_once(|| {
        let sandbox = std::env::temp_dir().join("flux-fuzz-menu-cfg");
        std::env::set_var("XDG_CONFIG_HOME", &sandbox);
    });

    if let Ok(s) = std::str::from_utf8(data) {
        let menu_path = dirs::config_dir()
            .unwrap_or_else(|| std::env::temp_dir().join("flux-fuzz-menu-cfg"))
            .join("flux/menu.rs");

        if let Some(parent) = menu_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&menu_path, s).is_ok() {
            let actions = flux::utils::load_menu_config();

            // Roundtrip: every parsed action must re-serialize and re-parse
            // without changing the number of entries. Guards against the
            // DSL parser silently dropping entries it just accepted.
            if flux::utils::save_menu_config(&actions).is_ok() {
                let actions2 = flux::utils::load_menu_config();
                assert_eq!(
                    actions.len(),
                    actions2.len(),
                    "menu.rs roundtrip lost entries"
                );
            }
        }
    }
});
