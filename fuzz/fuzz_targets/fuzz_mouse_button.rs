#![no_main]
use flux::utils::helpers::parse_mouse_button;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = parse_mouse_button(s);
    }
});
