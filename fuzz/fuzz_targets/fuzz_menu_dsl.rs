#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if let Ok(mut file) = tempfile::Builder::new().suffix(".rs").tempfile() {
        if file.write_all(data).is_ok() {
            if let Ok(content) = std::fs::read_to_string(file.path()) {
                for line in content.lines() {
                    if let Some((_, right)) = line.split_once("=>") {
                        let _ = flux::utils::split_mime_cmd(right);
                    }
                }
            }
        }
    }
});
