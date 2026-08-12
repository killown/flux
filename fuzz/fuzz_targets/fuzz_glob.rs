#![no_main]
use flux::utils::glob::glob_match;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let char_count = s.chars().count();
        let mid_char = char_count / 2;
        let mid = s.char_indices().nth(mid_char).map(|(i, _)| i).unwrap_or(0);
        let pattern = &s[..mid];
        let name = &s[mid..];
        let _ = glob_match(pattern, name);
    }
});
