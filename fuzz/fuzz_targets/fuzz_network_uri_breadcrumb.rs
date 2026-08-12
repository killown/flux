#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let uri = s.trim_end_matches('/');
        if let Some((_, after_scheme)) = uri.split_once("://") {
            let slash_pos = after_scheme.find('/');
            let authority_end = slash_pos.unwrap_or(after_scheme.len());
            let _authority = &after_scheme[..authority_end];
            if let Some(p) = slash_pos {
                let _path_part = &after_scheme[p + 1..];
            }
        }
    }
});
