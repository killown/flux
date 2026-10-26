#![no_main]
use flux::infra::tasks::{format_bytes, format_duration};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    let n = u64::from_le_bytes(data[..8].try_into().unwrap());

    let bytes = format_bytes(n);
    assert!(
        [" B", " KB", " MB", " GB"]
            .iter()
            .any(|u| bytes.ends_with(u)),
        "unexpected unit in {bytes:?}"
    );

    let text = format_duration(n);
    let parts: Vec<u128> = text
        .split(':')
        .map(|p| p.parse().expect("numeric field"))
        .collect();
    let total = match parts.as_slice() {
        [m, s] => {
            assert!(*m < 60 && *s < 60, "out-of-range field in {text:?}");
            m * 60 + s
        }
        [h, m, s] => {
            assert!(*h > 0 && *m < 60 && *s < 60, "bad fields in {text:?}");
            h * 3600 + m * 60 + s
        }
        _ => panic!("unexpected shape: {text:?}"),
    };
    assert_eq!(total, n as u128, "format_duration must round-trip");
});
