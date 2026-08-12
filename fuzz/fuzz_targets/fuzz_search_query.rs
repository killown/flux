#![no_main]
use flux::utils::search::parse_content_search_query;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = parse_content_search_query(s);
    }
});
