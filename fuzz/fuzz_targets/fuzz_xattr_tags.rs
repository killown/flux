#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if let Ok(mut file) = tempfile::NamedTempFile::new() {
        let tags: Vec<String> = std::str::from_utf8(data)
            .unwrap_or("")
            .split(',')
            .map(|s| s.to_string())
            .collect();
        let _ = flux::utils::xattr::write_tags(file.path(), &tags);
        let _ = flux::utils::xattr::read_tags(file.path());
    }
});
