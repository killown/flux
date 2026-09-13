#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if let Ok(mut file) = tempfile::Builder::new().suffix(".png").tempfile() {
        if file.write_all(data).is_ok() {
            let _ = flux::utils::media::probe_image_dimensions(file.path());
        }
    }
});
