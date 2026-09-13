#![no_main]
use flux::services::network::{is_network_uri, protocol_for_uri};
use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = protocol_for_uri(s);
        let _ = is_network_uri(Path::new(s));
    }
});
