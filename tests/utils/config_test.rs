use flux::model::{CustomAction, MenuEntry};
use flux::utils::config::{
    ensure_config_file, get_system_mounts, load_menu_config, remove_recents, rename_path,
    save_menu_config, split_mime_cmd,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::{tempdir, TempDir};

// Global lock to prevent parallel env variable race conditions across test threads
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_rename_path_rejects_path_separator() {
    let tmp = TempDir::new().unwrap();

    let file = tmp.path().join("original.txt");
    fs::write(&file, b"").unwrap();

    let err = rename_path(&file, "sub/dir/name.txt").unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn test_rename_path_rejects_existing_destination() {
    let tmp = TempDir::new().unwrap();

    let src = tmp.path().join("a.txt");
    let dst = tmp.path().join("b.txt");
    fs::write(&src, b"").unwrap();
    fs::write(&dst, b"").unwrap();

    let err = rename_path(&src, "b.txt").unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
}

#[test]
fn test_rename_path_happy_path() {
    let tmp = TempDir::new().unwrap();

    let src = tmp.path().join("old.txt");
    fs::write(&src, b"content").unwrap();

    let new_path = rename_path(&src, "new.txt").unwrap();
    assert!(!src.exists());
    assert!(new_path.exists());
    assert_eq!(new_path.file_name().unwrap(), "new.txt");
}

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
    use super::*;

    fn setup_xbel(dir: &TempDir, lines: &[&str]) -> PathBuf {
        let xbel_path = dir.path().join("recently-used.xbel");
        let content = lines.join("\n");
        fs::write(&xbel_path, content).unwrap();
        xbel_path
    }

    #[test]
    fn remove_recents_without_paths_clears_all_bookmarks() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let original_xdg = std::env::var_os("XDG_DATA_HOME");

        std::env::set_var("XDG_DATA_HOME", dir.path());

        let xbel_content = vec![
            r#"<?xml version="1.0"?>"#,
            r#"<xbel version="1.0">"#,
            r#"  <bookmark href="file:///tmp/file1.txt" modified="2025-01-01T00:00:00Z"/>"#,
            r#"  <bookmark href="file:///tmp/file2.txt" modified="2025-01-02T00:00:00Z"/>"#,
            r#"</xbel>"#,
        ];
        setup_xbel(&dir, &xbel_content);

        let result = remove_recents(None);
        assert!(result.is_ok());

        let content = fs::read_to_string(dir.path().join("recently-used.xbel")).unwrap();
        assert!(!content.contains("<bookmark"));
        assert!(content.contains(r#"<?xml version="1.0"?>"#));

        if let Some(val) = original_xdg {
            std::env::set_var("XDG_DATA_HOME", val);
        } else {
            std::env::remove_var("XDG_DATA_HOME");
        }
    }

    #[test]
    fn remove_recents_with_paths_removes_matching_entries_only() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let original_xdg = std::env::var_os("XDG_DATA_HOME");

        std::env::set_var("XDG_DATA_HOME", dir.path());

        let xbel_content = vec![
            r#"<?xml version="1.0"?>"#,
            r#"<xbel version="1.0">"#,
            r#"  <bookmark href="file:///tmp/file1.txt"/>"#,
            r#"  <bookmark href="file:///tmp/file2.txt"/>"#,
            r#"  <bookmark href="file:///tmp/file3.txt"/>"#,
            r#"</xbel>"#,
        ];
        setup_xbel(&dir, &xbel_content);

        let paths_to_remove = vec![
            PathBuf::from("/tmp/file1.txt"),
            PathBuf::from("/tmp/file3.txt"),
        ];
        let result = remove_recents(Some(&paths_to_remove));
        assert!(result.is_ok());

        let content = fs::read_to_string(dir.path().join("recently-used.xbel")).unwrap();
        assert!(content.contains("file2.txt"));
        assert!(!content.contains("file1.txt"));
        assert!(!content.contains("file3.txt"));

        if let Some(val) = original_xdg {
            std::env::set_var("XDG_DATA_HOME", val);
        } else {
            std::env::remove_var("XDG_DATA_HOME");
        }
    }

    #[test]
    fn remove_recents_handles_missing_xbel() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let original_xdg = std::env::var_os("XDG_DATA_HOME");

        std::env::set_var("XDG_DATA_HOME", dir.path());

        let result = remove_recents(None);
        assert!(result.is_ok());

        if let Some(val) = original_xdg {
            std::env::set_var("XDG_DATA_HOME", val);
        } else {
            std::env::remove_var("XDG_DATA_HOME");
        }
    }

    #[test]
    fn remove_recents_handles_malformed_xbel() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let original_xdg = std::env::var_os("XDG_DATA_HOME");

        std::env::set_var("XDG_DATA_HOME", dir.path());

        let malformed = vec![r#"<xbel>"#, r#"  <bookmark href="file:///tmp/a.txt"/>"#];
        setup_xbel(&dir, &malformed);

        let result = remove_recents(None);
        assert!(result.is_ok());

        let content = fs::read_to_string(dir.path().join("recently-used.xbel")).unwrap();
        assert!(!content.contains("<bookmark"));

        if let Some(val) = original_xdg {
            std::env::set_var("XDG_DATA_HOME", val);
        } else {
            std::env::remove_var("XDG_DATA_HOME");
        }
    }

    #[test]
    fn test_split_mime_cmd_malformed_inputs() {
        use flux::utils::config::split_mime_cmd;

        assert!(split_mime_cmd("all\", \"builtin::copy\"").is_none());

        assert!(split_mime_cmd("\"all\", \"builtin::copy").is_none());

        assert!(split_mime_cmd("").is_none());
    }

    #[test]
    fn test_get_mime_type_case_insensitive_extension() {
        use flux::utils::config::get_mime_type;
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
fn test_rename_path_rejects_slash_in_name() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("original.txt");
    fs::write(&file, b"content").unwrap();

    assert!(rename_path(&file, "../../etc/passwd").is_err());
    assert!(rename_path(&file, "subdir/file.txt").is_err());
    assert!(
        file.exists(),
        "source must be untouched after rejected rename"
    );
}

#[test]
fn test_rename_path_rejects_already_exists() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("original.txt");
    let existing = dir.path().join("existing.txt");
    fs::write(&file, b"content").unwrap();
    fs::write(&existing, b"other").unwrap();

    let result = rename_path(&file, "existing.txt");
    assert!(result.is_err());
    assert!(file.exists(), "source must be untouched");
    assert!(existing.exists(), "target must be untouched");
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
    use flux::utils::config::get_mime_type;
    use std::path::Path;
    assert_eq!(get_mime_type(Path::new("/tmp")), "inode/directory");
}

#[test]
fn get_mime_type_known_image_extensions() {
    use flux::utils::config::get_mime_type;
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
    use flux::utils::config::get_mime_type;
    use std::path::Path;
    assert_eq!(
        get_mime_type(Path::new("/tmp/x.zzzunknown")),
        "application/octet-stream"
    );
}

#[test]
fn expand_path_absolute_passthrough() {
    use flux::utils::config::expand_path;
    assert_eq!(
        expand_path("/absolute/path"),
        PathBuf::from("/absolute/path")
    );
}

#[test]
fn expand_path_relative_passthrough() {
    use flux::utils::config::expand_path;
    assert_eq!(expand_path("relative/path"), PathBuf::from("relative/path"));
}

#[test]
fn default_xdg_folder_icon_home_returns_user_home() {
    use flux::utils::config::get_default_xdg_folder_icon;
    if let Some(h) = dirs::home_dir() {
        assert_eq!(get_default_xdg_folder_icon(&h), Some("user-home"));
    }
}

#[test]
fn default_xdg_folder_icon_unknown_dir_returns_none() {
    use flux::utils::config::get_default_xdg_folder_icon;
    use std::path::Path;
    assert!(get_default_xdg_folder_icon(Path::new("/no/such/xyzzy")).is_none());
}

#[test]
fn rename_path_empty_name_rejected() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("file.txt");
    fs::write(&f, b"x").unwrap();
    assert!(rename_path(&f, "").is_err());
}

#[test]
fn rename_path_dotdot_rejected() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("file.txt");
    fs::write(&f, b"x").unwrap();
    assert!(rename_path(&f, "..").is_err());
}

#[test]
fn rename_path_unicode_name_accepted() {
    let dir = tempdir().unwrap();
    let f = dir.path().join("old.txt");
    fs::write(&f, b"x").unwrap();
    let p = rename_path(&f, "novo-文档.txt").unwrap();
    assert_eq!(p.file_name().unwrap().to_str().unwrap(), "novo-文档.txt");
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
    let result = flux::utils::config::resolve_folder_icon_with_fallbacks(&theme, "");
    assert!(result.is_some());
}
