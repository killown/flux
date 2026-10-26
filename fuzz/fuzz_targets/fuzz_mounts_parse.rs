#![no_main]
use flux::infra::mounts::{parse_fstab, parse_mountinfo, unescape_mount_field};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let decoded = unescape_mount_field(s);
        if !s.contains('\\') {
            assert_eq!(decoded, s, "unescape must be identity without escapes");
        }

        let mountinfo = parse_mountinfo(s);
        assert!(
            mountinfo.len() <= s.lines().count(),
            "parse_mountinfo returned more entries than lines"
        );

        for mount_point in parse_fstab(s) {
            assert!(
                mount_point.starts_with('/'),
                "parse_fstab produced a non-absolute entry: {mount_point:?}"
            );
        }
    }
});
