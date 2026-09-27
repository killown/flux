#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::{Component, Path, PathBuf};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Property 1: encode/decode is an identity on any path.
        let original = PathBuf::from(s);
        let encoded = flux::services::archive::encode_archive_host(&original);
        let decoded = flux::services::archive::decode_archive_host(&encoded);
        assert_eq!(
            decoded, original,
            "encode_archive_host → decode_archive_host must be identity"
        );

        // Property 2: build/parse round-trip preserves the archive path.
        let inner = "";
        let uri = flux::services::archive::build_archive_uri(&original, inner);
        if let Some((parsed_path, _parsed_inner)) =
            flux::services::archive::parse_archive_uri(&uri.to_string_lossy())
        {
            assert_eq!(
                parsed_path, original,
                "build_archive_uri → parse_archive_uri must preserve archive path"
            );
        }

        // Property 3: inner path returned by parse never escapes the archive root.
        for candidate in [
            format!("/archive://{}/../etc/passwd", encoded),
            format!("/archive://{}/a/../../b", encoded),
            format!("/archive://{}/./x", encoded),
        ] {
            if let Some((_, inner)) = flux::services::archive::parse_archive_uri(&candidate) {
                let inner_path = Path::new(&inner);
                for comp in inner_path.components() {
                    assert!(
                        !matches!(
                            comp,
                            Component::ParentDir | Component::RootDir | Component::Prefix(_)
                        ),
                        "parsed inner path must not contain traversal components: {:?}",
                        inner
                    );
                }
            }
        }
    }
});
