use flux::model::{Config, UIConfig};
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
