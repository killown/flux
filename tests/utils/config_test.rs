use flux::utils::config::{
    ensure_config_file, get_system_mounts, load_menu_config, remove_recents, rename_path,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::TempDir;

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
}
