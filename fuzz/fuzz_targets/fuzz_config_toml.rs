#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(config_str) = std::str::from_utf8(data) {
        let _: Result<flux::model::Config, _> = toml::from_str(config_str);
    }
});
