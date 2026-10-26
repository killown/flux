#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Err(msg) = flux::args::parse_icon_pair(s) {
            assert!(!msg.is_empty(), "error message must not be empty");
        }
    }
});
