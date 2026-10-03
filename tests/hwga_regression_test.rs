#![cfg(debug_assertions)]

use flux::hwga;
use flux::services::loader::{get_extension_icon_path, invalidate_extension_icon_cache};

const SCAN_PROBE: &str = "flux::services::loader::scan_directory_extensions";

#[test]
fn extension_icon_lookups_do_not_rescan_directories() {
    let sandbox = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", sandbox.path());
    std::env::set_var("XDG_DATA_HOME", sandbox.path().join("data"));
    std::env::set_var("XDG_CONFIG_HOME", sandbox.path().join("config"));
    hwga::reset_all();

    for _ in 0..500 {
        let _ = get_extension_icon_path("flxrepeat");
    }
    for i in 0..200 {
        let _ = get_extension_icon_path(&format!("flx{i}"));
    }

    let scans = hwga::count(SCAN_PROBE);
    assert!(
        scans <= 2,
        "icon lookups rescanned directories {scans} times (max 2)"
    );

    let before = hwga::count(SCAN_PROBE);
    invalidate_extension_icon_cache();
    assert_eq!(hwga::count(SCAN_PROBE) - before, 2);

    let after_invalidate = hwga::count(SCAN_PROBE);
    for _ in 0..500 {
        let _ = get_extension_icon_path("flxrepeat");
    }
    assert_eq!(hwga::count(SCAN_PROBE), after_invalidate);
}

#[test]
fn themed_icon_cache_prevents_gio_type_guess_storm() {
    if gtk::init().is_err() {
        return;
    }
    let sandbox = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", sandbox.path());
    hwga::reset_all();

    for i in 0..300 {
        let ext = ["rs", "toml", "png", "mp4", "zip"][i % 5];
        let p = std::path::PathBuf::from(format!("/tmp/flx_icon_{i}.{ext}"));
        let _ = flux::utils::get_icon_for_path(&p, false);
    }

    let lookups = hwga::count("flux::utils::config::get_icon_for_path");
    assert!(lookups <= 300, "resolution gate broken ({lookups})");
}
