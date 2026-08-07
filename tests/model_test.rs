use flux::model::{
    Config, CustomPlace, DeviceRename, MenuEntry, ShortcutsConfig, SortBy, ThumbnailTypes, UIConfig,
};
use std::collections::HashMap;

#[test]
fn test_sort_by_default() {
    let sort = SortBy::default();
    assert_eq!(sort, SortBy::Name);
}

#[test]
fn test_sort_by_serialization() {
    let config = Config {
        ui: UIConfig {
            default_sort: SortBy::Date,
            ..Default::default()
        },
        ..Default::default()
    };

    let toml_str = toml::to_string(&config).expect("Failed to serialize config");
    assert!(toml_str.contains("default_sort = \"Date\""));

    let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
    assert_eq!(parsed.ui.default_sort, SortBy::Date);
}

#[test]
fn test_config_defaults() {
    let empty_toml = "";
    let config: Config = toml::from_str(empty_toml).expect("Failed to parse empty config");

    assert_eq!(config.ui.default_icon_size, 0);
    assert!(!config.ui.single_click);
    assert_eq!(config.ui.default_sort, SortBy::Name);
    assert!(config.ui.folders_first);
}

#[test]
fn test_shortcuts_config_serialization() {
    let mut shortcuts = ShortcutsConfig::default();
    shortcuts.back = Some("BackSpace".to_string());
    shortcuts.forward = Some("<Alt>Right".to_string());

    let config = Config {
        shortcuts,
        ..Default::default()
    };

    let toml_str = toml::to_string(&config).expect("Failed to serialize config");
    assert!(toml_str.contains("back = \"BackSpace\""));
    assert!(toml_str.contains("forward = \"<Alt>Right\""));

    let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
    assert_eq!(parsed.shortcuts.back, Some("BackSpace".to_string()));
    assert_eq!(parsed.shortcuts.forward, Some("<Alt>Right".to_string()));
}

#[test]
fn test_custom_place_serialization() {
    let place = CustomPlace {
        name: "Home".to_string(),
        icon: "user-home-symbolic".to_string(),
        path: "~".to_string(),
        kind: None,
    };

    let config = Config {
        sidebar: vec![place],
        ..Default::default()
    };

    let toml_str = toml::to_string(&config).expect("Failed to serialize config");
    assert!(toml_str.contains("name = \"Home\""));
    assert!(toml_str.contains("path = \"~\""));

    let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
    assert_eq!(parsed.sidebar.len(), 1);
    assert_eq!(parsed.sidebar[0].name, "Home");
}

#[test]
fn test_device_rename_serialization() {
    let mut renames = HashMap::new();
    renames.insert(
        "/dev/sda1".to_string(),
        DeviceRename {
            name: "My Disk".to_string(),
            icon: Some("drive-harddisk-symbolic".to_string()),
        },
    );

    let config = Config {
        ui: UIConfig {
            device_renames: renames,
            ..Default::default()
        },
        ..Default::default()
    };

    let toml_str = toml::to_string(&config).expect("Failed to serialize config");
    assert!(toml_str.contains("My Disk"));

    let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
    let renamed = parsed.ui.device_renames.get("/dev/sda1").unwrap();
    assert_eq!(renamed.name, "My Disk");
    assert_eq!(renamed.icon, Some("drive-harddisk-symbolic".to_string()));
}

#[test]
fn test_folder_icons_serialization() {
    let mut icons = HashMap::new();
    icons.insert(
        "/home/user/Projects".to_string(),
        "folder-development-symbolic".to_string(),
    );

    let config = Config {
        ui: UIConfig {
            folder_icons: icons,
            ..Default::default()
        },
        ..Default::default()
    };

    let toml_str = toml::to_string(&config).expect("Failed to serialize config");
    assert!(toml_str.contains("folder-development-symbolic"));

    let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
    assert_eq!(
        parsed.ui.folder_icons.get("/home/user/Projects").cloned(),
        Some("folder-development-symbolic".to_string())
    );
}

#[test]
fn test_thumbnail_types_defaults() {
    let types = ThumbnailTypes::default();
    assert!(types.images);
    assert!(types.videos);
    assert!(types.fonts);
    assert!(types.pdfs);
}

#[test]
fn test_thumbnail_types_serialization() {
    let config = Config {
        ui: UIConfig {
            show_thumbnails: false,
            thumbnail_types: ThumbnailTypes {
                images: false,
                videos: true,
                fonts: false,
                pdfs: true,
            },
            ..Default::default()
        },
        ..Default::default()
    };

    let toml_str = toml::to_string(&config).expect("Failed to serialize config");
    assert!(toml_str.contains("show_thumbnails = false"));
    assert!(toml_str.contains("images = false"));
    assert!(toml_str.contains("videos = true"));
    assert!(toml_str.contains("fonts = false"));
    assert!(toml_str.contains("pdfs = true"));

    let parsed: Config = toml::from_str(&toml_str).expect("Failed to deserialize config");
    assert!(!parsed.ui.show_thumbnails);
    assert!(!parsed.ui.thumbnail_types.images);
    assert!(parsed.ui.thumbnail_types.videos);
    assert!(!parsed.ui.thumbnail_types.fonts);
    assert!(parsed.ui.thumbnail_types.pdfs);
}

#[test]
fn test_sort_by_action_key_roundtrip() {
    let variants = [SortBy::Name, SortBy::Date, SortBy::Size, SortBy::Type];
    for sort in variants {
        let variant = sort.as_action_state();
        let key = variant.str().expect("GVariant should be a string");
        let reconstructed = SortBy::from_action_key(key);
        assert_eq!(sort, reconstructed);
    }
}

#[test]
fn test_sort_by_fallback_on_unknown_key() {
    assert_eq!(SortBy::from_action_key("unknown_key"), SortBy::Name);
    assert_eq!(SortBy::from_action_key(""), SortBy::Name);
}

#[test]
fn test_menu_entry_to_config_line_variations() {
    let simple_entry = MenuEntry {
        label: "Open Terminal".to_string(),
        submenu: None,
        mime_types: "directory".to_string(),
        command: "alacritty".to_string(),
        toast: None,
    };
    assert_eq!(
        simple_entry.to_config_line(),
        r#""Open Terminal" => "directory", "alacritty""#
    );

    let nested_entry = MenuEntry {
        label: "To MP4".to_string(),
        submenu: Some("Media Convert".to_string()),
        mime_types: "video/all".to_string(),
        command: "ffmpeg -i %p %p.mp4".to_string(),
        toast: Some("Converting...".to_string()),
    };
    assert_eq!(
        nested_entry.to_config_line(),
        r#""Media Convert > To MP4" => "video/all", "ffmpeg -i %p %p.mp4", "Converting...""#
    );
}

#[test]
fn test_partial_ui_config_deserialization() {
    let partial_toml = r#"
        [ui]
        sidebar_width = 250
        single_click = true
    "#;
    let config: Config = toml::from_str(partial_toml).expect("Must parse partial config");
    assert_eq!(config.ui.sidebar_width, 250);
    assert!(config.ui.single_click);
    assert!(config.ui.folders_first);
    assert!(config.ui.show_thumbnails);
}
