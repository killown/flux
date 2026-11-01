//! `parse_accel_opt`: how a `config.toml` shortcut value becomes the
//! accelerator an app-level action is bound to, including `"disabled"`.

use flux::ui::keymap::{parse_accel_opt, DISABLED};

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

#[test]
fn an_unset_shortcut_uses_the_default() {
    assert_eq!(parse_accel_opt(&None, "<Primary>n"), some("<Primary>n"));
}

#[test]
fn a_user_shortcut_replaces_the_default_and_is_trimmed() {
    assert_eq!(
        parse_accel_opt(&some("<Alt>x"), "<Primary>n"),
        some("<Alt>x")
    );
    assert_eq!(
        parse_accel_opt(&some("  <Alt>x \n"), "<Primary>n"),
        some("<Alt>x")
    );
}

#[test]
fn a_blank_shortcut_falls_back_to_the_default() {
    assert_eq!(parse_accel_opt(&some(""), "F1"), some("F1"));
    assert_eq!(parse_accel_opt(&some("   "), "F1"), some("F1"));
}

#[test]
fn disabled_installs_no_accelerator_at_all() {
    assert_eq!(DISABLED, "disabled");
    assert_eq!(parse_accel_opt(&some(DISABLED), "<Primary>n"), None);
    assert_eq!(parse_accel_opt(&some("  disabled  "), "<Primary>n"), None);
}
