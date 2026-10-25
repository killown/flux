use flux::utils::helpers::editor_shell_command;
use std::path::Path;

#[test]
fn path_is_single_quoted() {
    let cmd = editor_shell_command("nvim +%l %p", Path::new("/tmp/a b.rs"), 42).unwrap();
    assert_eq!(cmd, "nvim +42 '/tmp/a b.rs'");
}

#[test]
fn command_substitution_in_filename_stays_inside_quotes() {
    let path = Path::new("/tmp/$(touch pwn)`id`\"x");
    let cmd = editor_shell_command("edit %p", path, 1).unwrap();
    assert_eq!(cmd, "edit '/tmp/$(touch pwn)`id`\"x'");
}

#[test]
fn single_quote_in_filename_is_escaped() {
    let cmd = editor_shell_command("edit %p", Path::new("/tmp/it's.rs"), 1).unwrap();
    assert_eq!(cmd, "edit '/tmp/it'\\''s.rs'");
}

#[test]
fn placeholders_inside_the_filename_are_not_expanded() {
    let cmd = editor_shell_command("edit +%l %p", Path::new("/tmp/%l%p"), 9).unwrap();
    assert_eq!(cmd, "edit +9 '/tmp/%l%p'");
}

#[test]
fn quotes_written_around_the_placeholder_are_absorbed() {
    let cmd = editor_shell_command("edit \"%p\"", Path::new("/tmp/x"), 1).unwrap();
    assert_eq!(cmd, "edit '/tmp/x'");
    let cmd = editor_shell_command("edit '%p'", Path::new("/tmp/x"), 1).unwrap();
    assert_eq!(cmd, "edit '/tmp/x'");
}

#[test]
fn newline_in_path_is_rejected() {
    assert!(editor_shell_command("edit %p", Path::new("/tmp/a\nb"), 1).is_none());
}

#[test]
fn editor_without_placeholders_does_not_use_the_shell() {
    assert!(editor_shell_command("nvim", Path::new("/tmp/x"), 1).is_none());
}
