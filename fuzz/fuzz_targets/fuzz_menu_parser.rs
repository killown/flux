#![no_main]
use libfuzzer_sys::fuzz_target;

use flux::utils::split_mime_cmd;

fuzz_target!(|data: &[u8]| {
    if let Ok(input_str) = std::str::from_utf8(data) {
        let _ = split_mime_cmd(input_str);
    }
});
