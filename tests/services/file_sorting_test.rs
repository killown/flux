#[test]
fn test_file_sorting_order() {
    struct MockEntry {
        name: &'static str,
        is_dir: bool,
    }

    let mut entries = [
        MockEntry {
            name: "zebra.txt",
            is_dir: false,
        },
        MockEntry {
            name: "documents",
            is_dir: true,
        },
        MockEntry {
            name: "apple.txt",
            is_dir: false,
        },
        MockEntry {
            name: "bin",
            is_dir: true,
        },
    ];

    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(b.name)));

    assert!(entries[0].is_dir);
    assert_eq!(entries[0].name, "bin");
    assert!(entries[1].is_dir);
    assert_eq!(entries[1].name, "documents");
    assert!(!entries[2].is_dir);
    assert_eq!(entries[2].name, "apple.txt");
    assert!(!entries[3].is_dir);
    assert_eq!(entries[3].name, "zebra.txt");
}

#[test]
fn test_file_name_filtering() {
    let files = vec!["main.rs", "lib.rs", "utils.rs", "readme.md", "Cargo.toml"];
    let query = ".rs";

    let matches: Vec<&str> = files
        .into_iter()
        .filter(|file| file.contains(query))
        .collect();

    assert_eq!(matches.len(), 3);
    assert!(matches.contains(&"main.rs"));
    assert!(matches.contains(&"lib.rs"));
    assert!(matches.contains(&"utils.rs"));
}
