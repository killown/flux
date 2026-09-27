#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::{Component, Path};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Feed arbitrary strings as if they were archive URIs and check that any
        // successfully-parsed inner path stays within the archive virtual root.
        if let Some((_archive, inner)) = flux::services::archive::parse_archive_uri(s) {
            for comp in Path::new(&inner).components() {
                assert!(
                    !matches!(
                        comp,
                        Component::ParentDir | Component::RootDir | Component::Prefix(_)
                    ),
                    "inner path escaped archive root: {:?}",
                    inner
                );
            }
        }
    }
});
