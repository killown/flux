use flux::utils::xdg_user_dirs::{parse_user_dirs, UserDir};
use std::path::{Path, PathBuf};

#[test]
fn parses_home_relative_and_absolute_entries() {
    let text = r#"
# comment line
XDG_DESKTOP_DIR="$HOME/Desktop"
XDG_PROJECTS_DIR="$HOME/Projects"
XDG_MUSIC_DIR="/mnt/media/music"
"#;
    let dirs = parse_user_dirs(text, Path::new("/home/u"));
    assert_eq!(
        dirs,
        vec![
            UserDir {
                key: "DESKTOP".into(),
                path: PathBuf::from("/home/u/Desktop")
            },
            UserDir {
                key: "PROJECTS".into(),
                path: PathBuf::from("/home/u/Projects")
            },
            UserDir {
                key: "MUSIC".into(),
                path: PathBuf::from("/mnt/media/music")
            },
        ]
    );
}

#[test]
fn skips_malformed_and_unsupported_lines() {
    let text = "XDG_=\"$HOME/x\"\nXDG_FOO_DIR=relative\nXDG_BAR_DIR=\"relative/path\"\nXDG_BAZ=\"$HOME/y\"\nNOT_XDG_DIR=\"$HOME/z\"\n";
    assert!(parse_user_dirs(text, Path::new("/home/u")).is_empty());
}

#[test]
fn home_value_resolves_to_home() {
    let dirs = parse_user_dirs("XDG_PUBLICSHARE_DIR=\"$HOME\"\n", Path::new("/home/u"));
    assert_eq!(dirs[0].path, PathBuf::from("/home/u"));
}
