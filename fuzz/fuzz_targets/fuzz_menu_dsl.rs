#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(_s) = std::str::from_utf8(data) {
        let _ = flux::utils::load_menu_config();
    }
});
