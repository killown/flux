use flux::utils::path::PathExt;
use std::env;
use std::path::PathBuf;

#[test]
fn test_expand_tilde_config_path() {
    let original_home = env::var_os("HOME");
    let mock_home = "/home/developer";
    env::set_var("HOME", mock_home);

    let path = PathBuf::from("~/.config/flux/config.toml");
    assert_eq!(
        path.expand_tilde(),
        PathBuf::from("/home/developer/.config/flux/config.toml")
    );

    if let Some(home) = original_home {
        env::set_var("HOME", home);
    }
}

#[test]
fn test_expand_tilde_real_user_fallback() {
    let real_home = env::var_os("HOME").map(PathBuf::from);

    if let Some(home_path) = real_home {
        let path = PathBuf::from("~/Downloads");
        let expected = home_path.join("Downloads");
        assert_eq!(path.expand_tilde(), expected);
    }
}

#[test]
fn test_expand_tilde_no_expansion_scenarios() {
    let path = PathBuf::from("/home/user/Documents/notes.txt~");
    assert_eq!(
        path.expand_tilde(),
        PathBuf::from("/home/user/Documents/notes.txt~")
    );

    let path = PathBuf::from("/var/www/html/site~backup/index.html");
    assert_eq!(
        path.expand_tilde(),
        PathBuf::from("/var/www/html/site~backup/index.html")
    );

    let path = PathBuf::from("/etc/flux~/settings.conf");
    assert_eq!(
        path.expand_tilde(),
        PathBuf::from("/etc/flux~/settings.conf")
    );
}

#[test]
fn test_relative_path_integrity() {
    let path = PathBuf::from("./local/file.txt");
    assert_eq!(path.expand_tilde(), PathBuf::from("./local/file.txt"));

    let path = PathBuf::from("../parent/file.txt");
    assert_eq!(path.expand_tilde(), PathBuf::from("../parent/file.txt"));
}

#[test]
fn test_expand_tilde_lone_tilde() {
    let original_home = env::var_os("HOME");
    env::set_var("HOME", "/home/developer");

    let path = PathBuf::from("~");
    assert_eq!(path.expand_tilde(), PathBuf::from("/home/developer"));

    if let Some(home) = original_home {
        env::set_var("HOME", home);
    }
}

#[test]
fn test_copy_dir_recursive_merging() {
    use flux::utils::helpers::copy_dir_recursive;
    use tempfile::tempdir;

    let src_dir = tempdir().unwrap();
    let dst_dir = tempdir().unwrap();

    let src_sub = src_dir.path().join("sub");
    std::fs::create_dir(&src_sub).unwrap();
    std::fs::write(src_sub.join("a.txt"), b"data A").unwrap();

    let dst_sub = dst_dir.path().join("sub");
    std::fs::create_dir(&dst_sub).unwrap();
    std::fs::write(dst_sub.join("b.txt"), b"data B").unwrap();

    copy_dir_recursive(src_dir.path(), dst_dir.path()).unwrap();

    assert!(dst_sub.join("a.txt").exists());
    assert!(dst_sub.join("b.txt").exists());
}

#[test]
fn test_run_custom_command_placeholder_escaping() {
    let file_path = PathBuf::from("/tmp/folder with spaces/file'name.txt");
    let cmd_template = "echo %p %d %f";

    let path_str = file_path.to_string_lossy();
    let parent = file_path.parent().unwrap_or(&file_path).to_string_lossy();
    let filename = file_path.file_name().unwrap_or_default().to_string_lossy();

    let p_arg = format!("'{}'", path_str.replace('\'', "'\\''"));
    let d_arg = format!("'{}'", parent.replace('\'', "'\\''"));
    let f_arg = format!("'{}'", filename.replace('\'', "'\\''"));

    let final_cmd = cmd_template
        .replace("%p", &p_arg)
        .replace("%d", &d_arg)
        .replace("%f", &f_arg);

    assert!(final_cmd.contains("'/tmp/folder with spaces/file'\\''name.txt'"));
}
