#![no_main]
use flux::services::archive::{decode_archive_host, encode_archive_host};
use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let decoded = decode_archive_host(s);
        let _ = encode_archive_host(&decoded);
        let encoded = encode_archive_host(Path::new(s));
        let _ = decode_archive_host(&encoded);
    }
});
