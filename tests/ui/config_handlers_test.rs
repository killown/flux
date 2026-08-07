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
