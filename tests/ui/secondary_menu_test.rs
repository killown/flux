use flux::ui::secondary_context_menu::{parse_secondary_template, resolve_secondary_menu_template};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_secondary_menu_template_priority_chain() {
    let tmp = TempDir::new().unwrap();
    let menus_dir = tmp.path().join("flux").join("menus");
    fs::create_dir_all(&menus_dir).unwrap();

    std::env::set_var("XDG_CONFIG_HOME", tmp.path());

    let p_sub = menus_dir.join("menu-image-png.rs");
    let p_cat = menus_dir.join("menu-image-all.rs");
    let p_fallback = menus_dir.join("menu-all.rs");

    fs::write(&p_fallback, r#""Test" => "all", "echo fallback""#).unwrap();
    assert_eq!(
        resolve_secondary_menu_template("image/png"),
        Some(p_fallback.clone())
    );

    fs::write(&p_cat, r#""Test" => "image/all", "echo cat""#).unwrap();
    assert_eq!(
        resolve_secondary_menu_template("image/png"),
        Some(p_cat.clone())
    );

    fs::write(&p_sub, r#""Test" => "image/png", "echo sub""#).unwrap();
    assert_eq!(
        resolve_secondary_menu_template("image/png"),
        Some(p_sub.clone())
    );

    std::env::remove_var("XDG_CONFIG_HOME");
}

#[test]
fn test_parse_secondary_template_submenus_and_naming() {
    let tmp = TempDir::new().unwrap();
    let template_file = tmp.path().join("menu-image-all.rs");

    let dsl = "\
# Comment line
// Another comment

\"Set Wallpaper\" => \"image/all\", \"swww img %p\", \"Wallpaper set\"
\"Convert > To WebP\" => \"image/all\", \"magick %p %p.webp\", \"Done\", \"no_command_dialog\"
";

    fs::write(&template_file, dsl).unwrap();

    let actions = parse_secondary_template(&template_file);

    assert_eq!(actions.len(), 2);

    assert_eq!(actions[0].label, "Set Wallpaper");
    assert_eq!(actions[0].submenu, None);
    assert_eq!(actions[0].action_name, "sec_menu_image_all_3");
    assert!(!actions[0].no_command_dialog);

    assert_eq!(actions[1].label, "To WebP");
    assert_eq!(actions[1].submenu, Some("Convert".to_string()));
    assert_eq!(actions[1].action_name, "sec_menu_image_all_4");
    assert!(actions[1].no_command_dialog);
}
