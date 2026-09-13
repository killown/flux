#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let parts: Vec<&str> = s.splitn(2, '\n').collect();
        let cmd = parts[0];
        let path_str = parts.get(1).copied().unwrap_or("/tmp/test file.txt");
        let path = PathBuf::from(path_str);
        let current_path = PathBuf::from("/tmp");

        let _ = flux::ui::file_ops::build_execution_command(cmd, &[path], &current_path);
    }
});
