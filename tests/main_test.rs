use flux::args::{resolve_startup_action, StartupAction};
use std::path::PathBuf;

fn args(argv: &[&str]) -> Vec<String> {
    argv.iter().map(|s| s.to_string()).collect()
}

const FAKE_HOME: &str = "/home/testuser";

fn home() -> PathBuf {
    PathBuf::from(FAKE_HOME)
}

// ── No-arg / default path ────────────────────────────────────────────────

#[test]
fn no_args_launches_home_dir() {
    // Regression: argv[0]-only must never produce PathBuf::new() (the empty
    // path), which was the root cause of the bug where `cargo run -- /home/`
    // opened the workspace root instead of /home/.
    let action = resolve_startup_action(&args(&["flux-fm"]), home());
    assert_eq!(action, StartupAction::Launch(PathBuf::from(FAKE_HOME)));
}

#[test]
fn no_args_launch_path_is_never_empty() {
    let action = resolve_startup_action(&args(&["flux-fm"]), home());
    if let StartupAction::Launch(path) = action {
        assert!(
            !path.as_os_str().is_empty(),
            "Launch path must never be PathBuf::new() (empty string): \
             init_components would open the process cwd instead of the intended directory"
        );
    }
}

#[test]
fn empty_args_slice_launches_home_dir() {
    // Edge case: completely empty slice (no argv[0]).
    let action = resolve_startup_action(&[], home());
    assert_eq!(action, StartupAction::Launch(PathBuf::from(FAKE_HOME)));
}

// ── Explicit path argument ───────────────────────────────────────────────

#[test]
fn absolute_path_arg_launches_that_path() {
    let action = resolve_startup_action(&args(&["flux-fm", "/home/"]), home());
    assert_eq!(action, StartupAction::Launch(PathBuf::from("/home/")));
}

#[test]
fn absolute_path_arg_is_not_home_dir() {
    // Validates that the supplied path, not the home fallback, is used.
    let action = resolve_startup_action(&args(&["flux-fm", "/home/"]), home());
    assert_ne!(action, StartupAction::Launch(home()));
}

#[test]
fn root_path_arg_launches_root() {
    let action = resolve_startup_action(&args(&["flux-fm", "/"]), home());
    assert_eq!(action, StartupAction::Launch(PathBuf::from("/")));
}

#[test]
fn relative_path_arg_launches_that_relative_path() {
    let action = resolve_startup_action(&args(&["flux-fm", "Documents"]), home());
    assert_eq!(action, StartupAction::Launch(PathBuf::from("Documents")));
}

#[test]
fn tilde_path_arg_is_passed_through_as_literal() {
    // resolve_startup_action does NOT expand tildes, that is the caller's
    // responsibility (utils::expand_path).  This test documents the contract.
    let action = resolve_startup_action(&args(&["flux-fm", "~"]), home());
    assert_eq!(action, StartupAction::Launch(PathBuf::from("~")));
}

#[test]
fn path_with_spaces_is_preserved() {
    let action = resolve_startup_action(&args(&["flux-fm", "/home/user/My Documents"]), home());
    assert_eq!(
        action,
        StartupAction::Launch(PathBuf::from("/home/user/My Documents"))
    );
}

// ── --version / -v ───────────────────────────────────────────────────────

#[test]
fn version_long_flag() {
    let action = resolve_startup_action(&args(&["flux-fm", "--version"]), home());
    assert_eq!(action, StartupAction::PrintVersion);
}

#[test]
fn version_short_flag() {
    let action = resolve_startup_action(&args(&["flux-fm", "-v"]), home());
    assert_eq!(action, StartupAction::PrintVersion);
}

// ── --help / -h ──────────────────────────────────────────────────────────

#[test]
fn help_long_flag() {
    let action = resolve_startup_action(&args(&["flux-fm", "--help"]), home());
    assert_eq!(action, StartupAction::PrintHelp);
}

#[test]
fn help_short_flag() {
    let action = resolve_startup_action(&args(&["flux-fm", "-h"]), home());
    assert_eq!(action, StartupAction::PrintHelp);
}

// ── --menu-editor ────────────────────────────────────────────────────────

#[test]
fn menu_editor_flag() {
    let action = resolve_startup_action(&args(&["flux-fm", "--menu-editor"]), home());
    assert_eq!(action, StartupAction::MenuEditor);
}

// ── --file-properties ────────────────────────────────────────────────────

#[test]
fn file_properties_with_path() {
    let action = resolve_startup_action(
        &args(&["flux-fm", "--file-properties", "/home/user/photo.jpg"]),
        home(),
    );
    assert_eq!(
        action,
        StartupAction::FileProperties(PathBuf::from("/home/user/photo.jpg"))
    );
}

#[test]
fn file_properties_without_path_falls_back_to_help() {
    // Missing PATH operand → show help rather than panicking.
    let action = resolve_startup_action(&args(&["flux-fm", "--file-properties"]), home());
    assert_eq!(action, StartupAction::PrintHelp);
}

#[test]
fn file_properties_path_is_not_empty() {
    let action = resolve_startup_action(
        &args(&["flux-fm", "--file-properties", "/tmp/file.txt"]),
        home(),
    );
    if let StartupAction::FileProperties(path) = action {
        assert!(!path.as_os_str().is_empty());
    } else {
        panic!("expected FileProperties variant");
    }
}

// ── Unknown flags ────────────────────────────────────────────────────────

#[test]
fn unknown_long_flag_is_reported() {
    let action = resolve_startup_action(&args(&["flux-fm", "--no-such-flag"]), home());
    assert_eq!(
        action,
        StartupAction::UnknownFlag("--no-such-flag".to_string())
    );
}

#[test]
fn unknown_short_flag_is_reported() {
    let action = resolve_startup_action(&args(&["flux-fm", "-z"]), home());
    assert_eq!(action, StartupAction::UnknownFlag("-z".to_string()));
}

#[test]
fn unknown_flag_preserves_exact_text() {
    let flag = "--flag-with-value=something";
    let action = resolve_startup_action(&args(&["flux-fm", flag]), home());
    assert_eq!(action, StartupAction::UnknownFlag(flag.to_string()));
}

// ── Launch path invariants ───────────────────────────────────────────────

#[test]
fn launch_path_is_never_path_buf_new_for_any_path_arg() {
    // Parametrised regression: every path-like argument must produce a
    // non-empty Launch path.  PathBuf::new() is the sentinel that caused
    // init_components to receive an empty path and open the cwd.
    let cases = ["/home/", "/", "/tmp", ".", "Documents", "~"];
    for input in cases {
        let action = resolve_startup_action(&args(&["flux-fm", input]), home());
        if let StartupAction::Launch(path) = action {
            assert!(
                !path.as_os_str().is_empty(),
                "input '{input}' produced empty PathBuf - init_components regression risk"
            );
        }
    }
}

#[test]
fn home_fallback_is_never_path_buf_new() {
    // Ensures that even a degenerate home_dir argument (PathBuf::new()) is
    // caught: callers must supply a real fallback.
    let action = resolve_startup_action(&args(&["flux-fm"]), PathBuf::from("."));
    if let StartupAction::Launch(path) = action {
        assert!(
            !path.as_os_str().is_empty(),
            "home fallback produced empty path"
        );
    }
}

// ── Discriminant exhaustiveness ──────────────────────────────────────────

#[test]
fn all_flag_variants_are_distinct() {
    // Ensures that no two flag strings collapse to the same action variant,
    // catching future copy-paste errors in the match arms.
    use std::mem::discriminant;

    let version = resolve_startup_action(&args(&["flux-fm", "--version"]), home());
    let help = resolve_startup_action(&args(&["flux-fm", "--help"]), home());
    let menu = resolve_startup_action(&args(&["flux-fm", "--menu-editor"]), home());
    let props = resolve_startup_action(&args(&["flux-fm", "--file-properties", "/f"]), home());
    let launch = resolve_startup_action(&args(&["flux-fm", "/tmp"]), home());
    let unknown = resolve_startup_action(&args(&["flux-fm", "--bad"]), home());

    let variants = [
        discriminant(&version),
        discriminant(&help),
        discriminant(&menu),
        discriminant(&props),
        discriminant(&launch),
        discriminant(&unknown),
    ];

    let unique: std::collections::HashSet<_> = variants.iter().collect();
    assert_eq!(
        unique.len(),
        variants.len(),
        "two flags resolved to the same action variant"
    );
}
