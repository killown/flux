#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::Path;
use flux::utils::PathExt;

fuzz_target!(|data: &[u8]| {
    if let Ok(path_str) = std::str::from_utf8(data) {
        let path = Path::new(path_str);
        let _ = path.expand_tilde();
    }
});
