#![no_main]
use flux::utils::helpers::is_recursive_paste;
use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let mut parts = s.splitn(2, '\n');
        let src = Path::new(parts.next().unwrap_or(""));
        let dest = Path::new(parts.next().unwrap_or(""));
        let _ = is_recursive_paste(src, dest);
    }
});
