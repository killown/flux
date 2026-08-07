#[test]
fn test_extension_filter_parsing() {
    let ext_filter = Some("rs, txt, TOML ".to_string());

    let allowed_exts: Option<Vec<String>> = ext_filter.as_ref().map(|s| {
        s.split(',')
            .map(|part| part.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    });

    let exts = allowed_exts.unwrap();
    assert_eq!(exts.len(), 3);
    assert_eq!(exts[0], "rs");
    assert_eq!(exts[1], "txt");
    assert_eq!(exts[2], "toml");
}

#[test]
fn test_extension_filter_matching() {
    let allowed_exts = vec!["rs".to_string(), "md".to_string()];

    let file1_ext = "rs";
    let file2_ext = "py";

    assert!(allowed_exts.contains(&file1_ext.to_string()));
    assert!(!allowed_exts.contains(&file2_ext.to_string()));
}

#[test]
fn test_content_line_search_and_line_number() {
    let content = "fn main() {\n    let search_term = true;\n    println!(\"hello\");\n}";
    let term_lc = "search_term";

    let mut found_line = None;
    let mut found_num = 0;

    for (line_number, line) in content.lines().enumerate() {
        if line.to_lowercase().contains(term_lc) {
            found_line = Some(line.trim().to_string());
            found_num = line_number + 1;
            break;
        }
    }

    assert_eq!(found_line, Some("let search_term = true;".to_string()));
    assert_eq!(found_num, 2);
}

#[test]
fn test_system_path_skip_check() {
    let system_paths = vec!["/proc/1/cmdline", "/sys/class/net", "/dev/null"];
    let normal_path = "/home/user/documents/file.txt";

    let should_skip = |path: &str| -> bool {
        path.starts_with("/proc/") || path.starts_with("/sys/") || path.starts_with("/dev/")
    };

    for path in system_paths {
        assert!(should_skip(path));
    }
    assert!(!should_skip(normal_path));
}
