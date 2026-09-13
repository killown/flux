#![no_main]
use flux::services::archive::{entries_to_load_contexts, ArchiveEntry};
use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
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
