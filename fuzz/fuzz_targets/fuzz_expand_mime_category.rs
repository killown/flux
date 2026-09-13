#![no_main]
use flux::utils::glob::expand_mime_category;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = expand_mime_category(s);
    }
});
