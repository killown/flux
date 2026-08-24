use flux::model::{Config, CustomPlace, UIConfig};
use std::path::PathBuf;

#[test]
fn test_shortcut_remapping_logic() {
    let mut config = Config::default();
    config.shortcuts.back = Some("BackSpace".to_string());
    config.shortcuts.search = Some("<Control>f".to_string());

    assert_eq!(config.shortcuts.back, Some("BackSpace".to_string()));
    assert_eq!(config.shortcuts.search, Some("<Control>f".to_string()));
}

#[test]
fn test_folder_icon_override_insertion() {
    let mut ui_config = UIConfig::default();
    let folder_path = PathBuf::from("/home/user/Projects");
    let path_key = folder_path.to_string_lossy().to_string();

    ui_config
        .folder_icons
        .insert(path_key.clone(), "folder-code".to_string());

    assert_eq!(
        ui_config.folder_icons.get(&path_key),
        Some(&"folder-code".to_string())
    );
}

#[test]
fn test_file_icon_override_insertion_and_removal() {
    let mut ui_config = UIConfig::default();
    let file_path = PathBuf::from("/home/user/song.mp3");
    let key = file_path.to_string_lossy().to_string();
    let image = "/home/user/art.jpg".to_string();

    ui_config.file_icons.insert(key.clone(), image.clone());
    assert_eq!(ui_config.file_icons.get(&key), Some(&image));

    ui_config.file_icons.remove(&key);
    assert!(ui_config.file_icons.get(&key).is_none());
}

#[test]
fn test_rename_sidebar_device_in_config() {
    use flux::model::{Config, DeviceRename};
    use std::collections::HashMap;
    use std::path::PathBuf;

    let mut config = Config::default();
    let mut renames = HashMap::new();
    renames.insert(
        "/mnt/vault".to_string(),
        DeviceRename {
            name: "Vault".to_string(),
            icon: Some("lock-symbolic".to_string()),
        },
    );
    config.ui.device_renames = renames;

    let target_path = PathBuf::from("/mnt/vault");
    let new_name = "Secure Vault".to_string();
    let path_str = target_path.to_string_lossy().to_string();

    let mut modified = false;
    if let Some(device) = config.ui.device_renames.get_mut(&path_str) {
        device.name = new_name;
        modified = true;
    }

    assert!(modified);
    assert_eq!(
        config.ui.device_renames.get("/mnt/vault").unwrap().name,
        "Secure Vault"
    );
}

#[test]
fn test_file_icon_override_update() {
    let mut ui_config = UIConfig::default();
    let key = "/home/user/doc.pdf".to_string();

    ui_config
        .file_icons
        .insert(key.clone(), "/img/v1.png".to_string());
    ui_config
        .file_icons
        .insert(key.clone(), "/img/v2.png".to_string());

    assert_eq!(
        ui_config.file_icons.get(&key).cloned(),
        Some("/img/v2.png".to_string())
    );
}

#[test]
fn test_set_folder_icon_updates_matching_sidebar_place() {
    let mut config = Config::default();
    let home = dirs::home_dir().unwrap_or_default();
    let target_dir = home.join("Downloads");

    config.sidebar.push(CustomPlace {
        name: "Downloads".into(),
        kind: None,
        icon: "folder-download-symbolic".into(),
        path: "~/Downloads".into(),
    });

    let new_icon = "folder-custom-symbolic".to_string();
    let path_str = target_dir.to_string_lossy().to_string();

    config.ui.folder_icons.insert(path_str, new_icon.clone());

    for place in &mut config.sidebar {
        let expanded = flux::utils::expand_path(&place.path);
        if expanded == target_dir {
            place.icon = new_icon.clone();
        }
    }

    assert_eq!(config.sidebar[0].icon, "folder-custom-symbolic");
    assert_eq!(
        config
            .ui
            .folder_icons
            .get(&target_dir.to_string_lossy().to_string()),
        Some(&"folder-custom-symbolic".to_string())
    );
}
