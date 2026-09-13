#![no_main]
use flux::ui::file_ops::build_execution_command;
use libfuzzer_sys::fuzz_target;
use std::path::{Path, PathBuf};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let lines: Vec<&str> = s.lines().collect();
        if let Some((template, rest)) = lines.split_first() {
            let targets: Vec<PathBuf> = rest.iter().map(PathBuf::from).collect();
            let cwd = Path::new("/tmp");
            let _ = build_execution_command(template, &targets, cwd);
        }
    }
});
