#![no_main]
use flux::utils::search::fuzzy_match;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let mut parts = s.splitn(2, '\n');
        let pattern = parts.next().unwrap_or("");
        let target = parts.next().unwrap_or("");

        let _ = fuzzy_match(target, pattern);

        if pattern.trim().is_empty() {
            assert!(fuzzy_match(target, pattern), "blank pattern must match");
        }
    }
});
