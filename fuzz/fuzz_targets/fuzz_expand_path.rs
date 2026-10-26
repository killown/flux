#![no_main]
use flux::utils::path::PathExt;
use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = Path::new(s).expand_tilde();
    }
});
