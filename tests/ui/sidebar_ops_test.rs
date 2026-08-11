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

#[test]
fn test_pin_folder_at_label() {
    let mut config = create_mock_config_with_labels();
    let home = dirs::home_dir().unwrap();
    let path = home.join("NewProject");
    let label_name = Some("Favorites".to_string());

    let home_str = home.to_string_lossy();
    let path_str = path.to_string_lossy().to_string();

    let resolve = |entry_path: &str| -> String {
        if entry_path.starts_with('~') {
            entry_path.replacen('~', &home_str, 1)
        } else {
            entry_path.to_owned()
        }
    };

    let already = config.sidebar.iter().any(|e| resolve(&e.path) == path_str);
    assert!(!already);

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path_str.clone());

    let new_entry = flux::model::CustomPlace {
        name,
        kind: None,
        icon: "folder-symbolic".to_string(),
        path: path_str.clone(),
    };

    let insert_at = if let Some(label_name) = label_name {
        config
            .sidebar
            .iter()
            .position(|e| e.kind.as_deref() == Some("label") && e.name == label_name)
            .unwrap_or(config.sidebar.len())
    } else {
        config
            .sidebar
            .iter()
            .position(|e| resolve(&e.path) == path_str)
            .unwrap_or(config.sidebar.len())
    };

    config.sidebar.insert(insert_at, new_entry);

    let inserted = &config.sidebar[insert_at];
    assert_eq!(inserted.path, path_str);
    if insert_at + 1 < config.sidebar.len() {
        let next = &config.sidebar[insert_at + 1];
        assert_eq!(next.kind, Some("label".to_string()));
        assert_eq!(next.name, "Favorites");
    } else {
        panic!("Insertion did not place before the label");
    }
}

#[test]
fn test_pin_folder_at_non_label() {
    let mut config = create_mock_config_with_labels();
    let home = dirs::home_dir().unwrap();
    let path = home.join("NewDownloads");
    let before = home.join("Projects");
    let label_name: Option<String> = None;

    let home_str = home.to_string_lossy();
    let path_str = path.to_string_lossy().to_string();
    let before_str = before.to_string_lossy().to_string();

    let resolve = |entry_path: &str| -> String {
        if entry_path.starts_with('~') {
            entry_path.replacen('~', &home_str, 1)
        } else {
            entry_path.to_owned()
        }
    };

    let already = config.sidebar.iter().any(|e| resolve(&e.path) == path_str);
    assert!(!already);

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path_str.clone());

    let new_entry = flux::model::CustomPlace {
        name,
        kind: None,
        icon: "folder-symbolic".to_string(),
        path: path_str.clone(),
    };

    let insert_at = if let Some(_) = label_name {
        unreachable!();
    } else {
        config
            .sidebar
            .iter()
            .position(|e| resolve(&e.path) == before_str)
            .unwrap_or(config.sidebar.len())
    };

    config.sidebar.insert(insert_at, new_entry);

    let inserted = &config.sidebar[insert_at];
    assert_eq!(inserted.path, path_str);
    if insert_at + 1 < config.sidebar.len() {
        let next = &config.sidebar[insert_at + 1];
        assert_eq!(resolve(&next.path), before_str);
    } else {
        panic!("Insertion did not place before the target row");
    }
}

#[test]
fn test_pin_folder_at_label_not_found_fallback_to_end() {
    let mut config = create_mock_config_with_labels();
    let home = dirs::home_dir().unwrap();
    let path = home.join("Misc");
    let label_name = Some("NonExistentLabel".to_string());

    let home_str = home.to_string_lossy();
    let path_str = path.to_string_lossy().to_string();

    let resolve = |entry_path: &str| -> String {
        if entry_path.starts_with('~') {
            entry_path.replacen('~', &home_str, 1)
        } else {
            entry_path.to_owned()
        }
    };

    let already = config.sidebar.iter().any(|e| resolve(&e.path) == path_str);
    assert!(!already);

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path_str.clone());

    let new_entry = flux::model::CustomPlace {
        name,
        kind: None,
        icon: "folder-symbolic".to_string(),
        path: path_str.clone(),
    };

    let insert_at = if let Some(label_name) = label_name {
        config
            .sidebar
            .iter()
            .position(|e| e.kind.as_deref() == Some("label") && e.name == label_name)
            .unwrap_or(config.sidebar.len())
    } else {
        unreachable!();
    };

    config.sidebar.insert(insert_at, new_entry);

    assert_eq!(insert_at, config.sidebar.len() - 1);
    let inserted = &config.sidebar[insert_at];
    assert_eq!(inserted.path, path_str);
}
fn create_mock_config_with_labels() -> Config {
    let mut config = Config::default();
    config.sidebar = vec![
        CustomPlace {
            name: "Home".into(),
            kind: None,
            icon: "folder".into(),
            path: "~".into(),
        },
        CustomPlace {
            name: "Favorites".into(),
            kind: Some("label".to_string()),
            icon: "".into(),
            path: "".into(),
        },
        CustomPlace {
            name: "Projects".into(),
            kind: None,
            icon: "folder".into(),
            path: "~/Projects".into(),
        },
        CustomPlace {
            name: "Downloads".into(),
            kind: None,
            icon: "folder".into(),
            path: "~/Downloads".into(),
        },
    ];
    config
}
