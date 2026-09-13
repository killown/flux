#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if let Ok(mut file) = tempfile::Builder::new().suffix(".rs").tempfile() {
        if file.write_all(data).is_ok() {
            let _ = flux::ui::secondary_context_menu::parse_secondary_template(file.path());
        }
    }
});
