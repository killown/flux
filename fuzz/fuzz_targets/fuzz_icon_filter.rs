#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(name_str) = std::str::from_utf8(data) {
        let is_symbolic = name_str.ends_with("-symbolic");
        if is_symbolic {
            assert!(name_str.len() >= 9);
        }
    }
});
