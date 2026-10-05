use flux::model::{Config, CustomPlace};

fn create_mock_config() -> Config {
    let mut config = Config::default();
    config.sidebar = vec![
        CustomPlace {
            name: "Home".into(),
            kind: None,
            icon: "folder".into(),
            path: "~".into(),
        },
        CustomPlace {
            name: "Downloads".into(),
            kind: None,
            icon: "folder".into(),
            path: "~/Downloads".into(),
        },
        CustomPlace {
            name: "Pictures".into(),
            kind: None,
            icon: "folder".into(),
            path: "~/Pictures".into(),
        },
    ];
    config
}

#[test]
fn test_handle_remove_from_sidebar_tilde_expansion() {
    let mut config = create_mock_config();
    let home = dirs::home_dir().unwrap_or_default();
    let downloads_path = home.join("Downloads");
    let path_str = downloads_path.to_string_lossy();

    // Replicate handle_remove_from_sidebar retention logic
    config.sidebar.retain(|entry| {
        let expanded = if entry.path.starts_with('~') {
            entry.path.replacen('~', &home.to_string_lossy(), 1)
        } else {
            entry.path.clone()
        };
        expanded != path_str.as_ref()
    });

    assert_eq!(config.sidebar.len(), 2);
    assert!(!config.sidebar.iter().any(|e| e.name == "Downloads"));
}

#[test]
fn test_handle_reorder_sidebar_shift() {
    let mut config = create_mock_config();
    let home = dirs::home_dir().unwrap_or_default();
    let home_str = home.to_string_lossy();

    let resolve = |entry_path: &str| -> String {
        if entry_path.starts_with('~') {
            entry_path.replacen('~', &home_str, 1)
        } else {
            entry_path.to_owned()
        }
    };

    let from_str = home.join("Pictures").to_string_lossy().to_string();
    let to_str = home.to_string_lossy().to_string();

    let from_idx = config
        .sidebar
        .iter()
        .position(|e| resolve(&e.path) == from_str);
    let to_idx = config
        .sidebar
        .iter()
        .position(|e| resolve(&e.path) == to_str);

    if let (Some(fi), Some(ti)) = (from_idx, to_idx) {
        let entry = config.sidebar.remove(fi);
        let insert_at = if fi < ti { ti - 1 } else { ti };
        config.sidebar.insert(insert_at, entry);
    }

    assert_eq!(config.sidebar[0].name, "Pictures");
    assert_eq!(config.sidebar[1].name, "Home");
    assert_eq!(config.sidebar[2].name, "Downloads");
}
