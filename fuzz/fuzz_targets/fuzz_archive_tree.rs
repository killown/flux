#![no_main]
use flux::services::archive::{entries_to_load_contexts, ArchiveEntry};
use libfuzzer_sys::fuzz_target;
use std::path::Path;
use std::sync::Once;

static INIT_FUZZ_ENV: Once = Once::new();

fuzz_target!(|data: &[u8]| {
    INIT_FUZZ_ENV.call_once(|| {
        let sandbox_cfg = std::env::temp_dir().join("flux-fuzz-cfg");
        let flux_cfg = sandbox_cfg.join("flux");
        let _ = std::fs::create_dir_all(&flux_cfg);

        // Turn off auto_generate_mime_icons so fuzz runs don't dump thousands of SVGs to disk
        let _ = std::fs::write(
            flux_cfg.join("config.toml"),
            b"[ui]\nauto_generate_mime_icons = false\n",
        );

        std::env::set_var("XDG_CONFIG_HOME", &sandbox_cfg);
        std::env::set_var("XDG_DATA_HOME", std::env::temp_dir().join("flux-fuzz-data"));
    });

    if let Ok(raw_listing) = std::str::from_utf8(data) {
        let entries: Vec<ArchiveEntry> = raw_listing
            .lines()
            .map(|line| {
                let is_dir = line.ends_with('/');
                ArchiveEntry {
                    name: line.trim_end_matches('/').to_string(),
                    is_dir,
                    size: line.len() as u64,
                    mtime: 0,
                    inner_path: line.to_string(),
                    child_count: 0,
                    is_encrypted: false,
                }
            })
            .collect();

        let archive_path = Path::new("/test/archive.tar.gz");
        let _ = entries_to_load_contexts(&entries, archive_path, false);
    }
});
