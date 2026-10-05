use flux::model::{CustomAction, MenuEntry};
use flux::services::mounts::get_system_mounts;
use flux::utils::config::{ensure_config_file, load_menu_config, save_menu_config, split_mime_cmd};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::TempDir;

// Global lock to prevent parallel env variable race conditions across test threads
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_ensure_config_file_creation() {
    let _guard = ENV_LOCK.lock().unwrap();

    let temp_dir = env::current_dir()
        .unwrap()
        .join("target")
        .join("test_config_init");

    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).unwrap();
    }
    fs::create_dir_all(&temp_dir).unwrap();

    env::set_var("XDG_CONFIG_HOME", &temp_dir);

    let path = ensure_config_file();
    assert!(path.exists());
    assert!(path.to_string_lossy().contains("flux"));

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_get_system_mounts_structure() {
    let mounts = get_system_mounts();

    assert!(!mounts.is_empty());

    for (name, path) in mounts {
        assert!(!name.is_empty(), "Mount name should not be empty");
        assert!(path.is_absolute(), "Mount path must be absolute");
    }
}

#[test]
fn test_config_invalid_toml() {
    let invalid_toml = "invalid = [unclosed bracket";

    let result: Result<flux::model::Config, _> = toml::from_str(invalid_toml);
    assert!(result.is_err());
}

#[test]
fn test_config_missing_fields() {
    let partial_toml = r#"
        [ui]
        sidebar_width = 300
    "#;

    let config: flux::model::Config = toml::from_str(partial_toml).unwrap_or_default();

    assert_eq!(config.ui.sidebar_width, 300);
    assert_eq!(config.ui.default_icon_size, 0);
}

#[test]
fn test_load_menu_config_integration() {
    let _guard = ENV_LOCK.lock().unwrap();
    let original_xdg = std::env::var_os("XDG_CONFIG_HOME");

    let tmp = TempDir::new().unwrap();
    let temp_dir = tmp.path();

    let flux_config_dir = temp_dir.join("flux");
    std::fs::create_dir_all(&flux_config_dir).unwrap();

    std::env::set_var("XDG_CONFIG_HOME", temp_dir);

    let config_path = flux_config_dir.join("menu.rs");
    let mock_content = r#""Copy" => "all", "builtin::copy", "Copied to clipboard""#;

    std::fs::write(config_path, mock_content).unwrap();

    let actions = load_menu_config();

    if let Some(val) = original_xdg {
        std::env::set_var("XDG_CONFIG_HOME", val);
    } else {
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    assert!(!actions.is_empty(), "Actions vector should not be empty");
    assert!(actions.iter().any(|a| a.label.contains("Copy")));
}

mod recents_tests {

    #[test]
    fn test_split_mime_cmd_malformed_inputs() {
        use flux::utils::config::split_mime_cmd;

        assert!(split_mime_cmd("all\", \"builtin::copy\"").is_none());

        assert!(split_mime_cmd("\"all\", \"builtin::copy").is_none());

        assert!(split_mime_cmd("").is_none());
    }

    #[test]
    fn test_get_mime_type_case_insensitive_extension() {
        use flux::utils::media::get_mime_type;
        use std::path::Path;

        let png_upper = Path::new("/nonexistent/file.PNG");
        let mime = get_mime_type(png_upper);
        assert_eq!(mime, "image/png");

        let unknown_ext = Path::new("/nonexistent/file.unknownext");
        let mime_unknown = get_mime_type(unknown_ext);
        assert_eq!(mime_unknown, "application/octet-stream");
    }
}

#[test]
fn test_menu_entry_no_command_dialog_serialization() {
    let entry = MenuEntry {
        label: "Test".to_string(),
        submenu: None,
        mime_types: "all".to_string(),
        command: "echo test".to_string(),
        toast: None,
        no_command_dialog: true,
    };
    let line = entry.to_config_line();
    assert!(line.contains(r#", "no_command_dialog""#));

    let entry2 = MenuEntry {
        no_command_dialog: false,
        ..entry.clone()
    };
    let line2 = entry2.to_config_line();
    assert!(!line2.contains(r#", "no_command_dialog""#));
}

#[test]
fn test_save_and_load_menu_config_with_no_command_dialog() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let config_dir = tmp_dir.path().join("flux");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", tmp_dir.path());

    let actions = vec![CustomAction {
        label: "Test".to_string(),
        submenu: None,
        action_name: "custom_0".to_string(),
        command: "echo test".to_string(),
        mime_types: vec!["all".to_string()],
        toast: None,
        no_command_dialog: true,
    }];

    let result = save_menu_config(&actions);
    assert!(result.is_ok());

    let loaded = load_menu_config();
    assert_eq!(loaded.len(), 1);
    assert!(loaded[0].no_command_dialog);

    std::env::remove_var("XDG_CONFIG_HOME");
}

#[test]
fn test_split_mime_cmd_no_command_dialog_flag() {
    let input = r#""all", "builtin::copy", "Copied", "no_command_dialog""#;
    let (mime, cmd, toast, no_dialog) = split_mime_cmd(input).expect("must parse");
    assert_eq!(mime, "all");
    assert_eq!(cmd, "builtin::copy");
    assert_eq!(toast.as_deref(), Some("Copied"));
    assert!(no_dialog);

    let input_no_flag = r#""all", "builtin::copy", "Copied""#;
    let (_, _, _, no_dialog2) = split_mime_cmd(input_no_flag).expect("must parse");
    assert!(!no_dialog2);

    let input_no_toast = r#""all", "builtin::copy", "no_command_dialog""#;
    let (_, _, toast3, no_dialog3) = split_mime_cmd(input_no_toast).expect("must parse");
    assert!(no_dialog3);
    assert!(toast3.is_none());
}

#[test]
fn test_split_mime_cmd_unicode_command() {
    use flux::utils::config::split_mime_cmd;
    let (_, cmd, _, _) = split_mime_cmd(r#""all", "echo café""#).unwrap();
    assert_eq!(cmd, "echo café");
}

#[test]
fn test_split_mime_cmd_empty_command() {
    use flux::utils::config::split_mime_cmd;
    let (_, cmd, _, _) = split_mime_cmd(r#""all", """#).unwrap();
    assert!(cmd.is_empty());
}

#[test]
fn test_split_mime_cmd_escaped_quote_in_command() {
    use flux::utils::config::split_mime_cmd;
    // Command with backslash-escaped inner quote
    let result = split_mime_cmd(r#""all", "echo \"x\"""#);
    assert!(result.is_some());
}

#[test]
fn test_split_mime_cmd_no_leading_quote_rejected() {
    use flux::utils::config::split_mime_cmd;
    assert!(split_mime_cmd("all, cmd").is_none());
}

#[test]
fn test_split_mime_cmd_trailing_content_after_quote() {
    use flux::utils::config::split_mime_cmd;
    assert!(split_mime_cmd(r#""all", "cmd" trailing garbage"#).is_some());
}

#[test]
fn test_split_mime_cmd_mime_with_slash() {
    use flux::utils::config::split_mime_cmd;
    let (mime, _, _, _) = split_mime_cmd(r#""image/png", "cmd""#).unwrap();
    assert_eq!(mime, "image/png");
}

#[test]
fn test_split_mime_cmd_mime_with_wildcard() {
    use flux::utils::config::split_mime_cmd;
    let (mime, _, _, _) = split_mime_cmd(r#""video/*", "cmd""#).unwrap();
    assert_eq!(mime, "video/*");
}

#[test]
fn test_split_mime_cmd_flag_only_no_toast() {
    use flux::utils::config::split_mime_cmd;
    let (_, _, toast, flag) = split_mime_cmd(r#""all", "cmd", "no_command_dialog""#).unwrap();
    assert!(flag);
    assert!(toast.is_none());
}

#[test]
fn get_mime_type_directory_returns_inode() {
    use flux::utils::media::get_mime_type;
    use std::path::Path;
    assert_eq!(get_mime_type(Path::new("/tmp")), "inode/directory");
}

#[test]
fn get_mime_type_known_image_extensions() {
    use flux::utils::media::get_mime_type;
    for ext in &["png", "jpg", "jpeg", "gif", "webp"] {
        let p = PathBuf::from(format!("/tmp/x.{}", ext));
        let mime = get_mime_type(&p);
        assert!(
            mime.starts_with("image/"),
            "expected image/* for .{}, got {}",
            ext,
            mime
        );
    }
}

#[test]
fn get_mime_type_unknown_ext_falls_back_to_octet_stream() {
    use flux::utils::media::get_mime_type;
    use std::path::Path;
    assert_eq!(
        get_mime_type(Path::new("/tmp/x.zzzunknown")),
        "application/octet-stream"
    );
}

#[test]
fn default_xdg_folder_icon_home_returns_user_home() {
    use flux::utils::icon::get_default_xdg_folder_icon;
    if let Some(h) = dirs::home_dir() {
        assert_eq!(get_default_xdg_folder_icon(&h), Some("user-home"));
    }
}

#[test]
fn default_xdg_folder_icon_unknown_dir_returns_none() {
    use flux::utils::icon::get_default_xdg_folder_icon;
    use std::path::Path;
    assert!(get_default_xdg_folder_icon(Path::new("/no/such/xyzzy")).is_none());
}

#[test]
fn ensure_config_file_returns_stable_path() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = TempDir::new().unwrap();
    std::env::set_var("XDG_CONFIG_HOME", tmp.path());
    let p1 = ensure_config_file();
    let p2 = ensure_config_file();
    assert_eq!(p1, p2);
    std::env::remove_var("XDG_CONFIG_HOME");
}

#[test]
fn resolve_folder_icon_empty_name_returns_some() {
    if gtk::init().is_err() {
        return;
    }
    let theme = gtk::IconTheme::for_display(&gtk::gdk::Display::default().unwrap());
    let result = flux::utils::icon::resolve_folder_icon_with_fallbacks(&theme, "");
    assert!(result.is_some());
}

#[test]
fn split_mime_cmd_no_command_dialog_before_toast() {
    use flux::utils::config::split_mime_cmd;
    let (m, c, t, f) = split_mime_cmd(r#""all", "echo", "no_command_dialog", "Copied""#).unwrap();
    assert_eq!(m, "all");
    assert_eq!(c, "echo");
    assert_eq!(t.as_deref(), Some("Copied"));
    assert!(f);
}

#[test]
fn split_mime_cmd_double_flag_only_last_wins() {
    use flux::utils::config::split_mime_cmd;
    let (_, _, _, f) =
        split_mime_cmd(r#""all", "echo", "no_command_dialog", "no_command_dialog""#).unwrap();
    assert!(f);
}
