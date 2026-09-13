#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let path = Path::new(s);
        let _ = flux::ui::conflict_policy::auto_rename_dest(path);
    }
});
