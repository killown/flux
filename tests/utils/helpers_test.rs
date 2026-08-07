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
