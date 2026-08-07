use std::path::PathBuf;

#[test]
fn test_filter_match_position_calculation() {
    let files = vec![
        ("document.pdf", PathBuf::from("/tmp/document.pdf")),
        ("doc_notes.txt", PathBuf::from("/tmp/doc_notes.txt")),
        ("image.png", PathBuf::from("/tmp/image.png")),
        ("my_doc.docx", PathBuf::from("/tmp/my_doc.docx")),
    ];

    let filter = "doc".to_lowercase();
    let pos = 1u32; // Target the second filtered match

    let mut match_count = 0u32;
    let mut found = None;

    for (name, path) in &files {
        if name.to_lowercase().contains(&filter) {
            if match_count == pos {
                found = Some(path.clone());
                break;
            }
            match_count += 1;
        }
    }

    assert_eq!(found, Some(PathBuf::from("/tmp/doc_notes.txt")));
}

#[test]
fn test_target_path_normalization_matching() {
    let items = vec![
        PathBuf::from("/home/user/folder/"),
        PathBuf::from("/home/user/file.txt"),
    ];

    let target_path = PathBuf::from("/home/user/folder");

    let target_normalized = target_path
        .to_string_lossy()
        .trim_end_matches('/')
        .to_string();

    let target_idx = items.iter().position(|p| {
        let item_normalized = p.to_string_lossy().trim_end_matches('/').to_string();
        item_normalized == target_normalized
    });

    assert_eq!(target_idx, Some(0));
}
