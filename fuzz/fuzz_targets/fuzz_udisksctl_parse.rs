#![no_main]
use flux::infra::luks::{
    is_wrong_passphrase, parse_dm_device, parse_loop_device, parse_mount_point,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Some(dev) = parse_loop_device(s) {
            assert!(dev.starts_with("/dev/loop"), "bad loop device: {dev:?}");
            assert!(!dev.ends_with('.'), "trailing dot kept: {dev:?}");
        }

        if let Some(dev) = parse_dm_device(s) {
            assert!(
                dev.starts_with("/dev/dm-") || dev.starts_with("/dev/mapper/"),
                "bad device-mapper node: {dev:?}"
            );
            assert!(!dev.ends_with('.'), "trailing dot kept: {dev:?}");
        }

        if let Some(mount_point) = parse_mount_point(s) {
            let text = mount_point.to_string_lossy();
            assert!(!text.is_empty(), "empty mount point returned");
            assert!(!text.ends_with('.'), "trailing dot kept: {text:?}");
        }

        if s.is_ascii() {
            assert_eq!(
                is_wrong_passphrase(s),
                is_wrong_passphrase(&s.to_ascii_uppercase()),
                "is_wrong_passphrase must be case-insensitive"
            );
        }
    }
});
