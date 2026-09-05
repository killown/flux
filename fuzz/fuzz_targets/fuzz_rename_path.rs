#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if let Ok(name) = std::str::from_utf8(data) {
        if let Ok(mut file) = tempfile::NamedTempFile::new() {
            let _ = flux::utils::config::rename_path(file.path(), name);
        }
    }
});
