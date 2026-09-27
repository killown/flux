#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Input format: "<filename>\n<ext1>,<ext2>,..."
        let mut parts = s.splitn(2, '\n');
        let name = parts.next().unwrap_or("");
        let ext_str = parts.next().unwrap_or("");

        let exts: Vec<String> = ext_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Both branches: file mode and directory early-return
        let _ = flux::utils::helpers::format_display_label(name, false, &exts);
        let _ = flux::utils::helpers::format_display_label(name, true, &exts);

        // Also seed the wildcard path explicitly (invariant: never panics)
        if !name.is_empty() {
            let _ = flux::utils::helpers::format_display_label(name, false, &["*".to_string()]);
            let _ = flux::utils::helpers::format_display_label(name, false, &["tar.*".to_string()]);
        }
    }
});
