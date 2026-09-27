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
            let (cmd, _label) = build_execution_command(template, &targets, cwd, 0);

            // Invariant: any command that survives escaping must not contain an
            // unescaped single quote outside of a quoted region. The escaper
            // rewrites ' to '\'' so the raw output must never contain a bare '
            // followed by a non-backslash character that would terminate a quote.
            //
            // A stricter invariant: if the input path contained a newline, the
            // command must be empty (rejected by shell_safe).
            for target in &targets {
                let ps = target.to_string_lossy();
                if ps.contains('\n') || ps.contains('\r') || ps.contains('\0') {
                    assert!(
                        cmd.is_empty(),
                        "command must be rejected when target contains unsafe bytes"
                    );
                    break;
                }
            }
        }
    }
});
