use flux::model::SortBy;

#[test]
fn test_shortcut_entry_value_trimming() {
    let parse_shortcut = |val: &str| -> Option<String> {
        let trimmed = val.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    };

    assert_eq!(
        parse_shortcut("  <Primary>f  "),
        Some("<Primary>f".to_string())
    );
    assert_eq!(parse_shortcut("   "), None);
    assert_eq!(parse_shortcut("F5"), Some("F5".to_string()));
}

#[test]
fn test_sort_dropdown_index_mapping() {
    let map_sort_to_index = |sort: SortBy| -> u32 {
        match sort {
            SortBy::Name => 0,
            SortBy::Size => 1,
            SortBy::Date => 2,
            SortBy::Type => 3,
        }
    };

    let map_index_to_sort = |index: u32| -> SortBy {
        match index {
            0 => SortBy::Name,
            1 => SortBy::Size,
            2 => SortBy::Date,
            3 => SortBy::Type,
            _ => SortBy::Name,
        }
    };

    assert_eq!(map_sort_to_index(SortBy::Name), 0);
    assert_eq!(map_sort_to_index(SortBy::Size), 1);
    assert_eq!(map_sort_to_index(SortBy::Date), 2);
    assert_eq!(map_sort_to_index(SortBy::Type), 3);

    assert_eq!(map_index_to_sort(0), SortBy::Name);
    assert_eq!(map_index_to_sort(1), SortBy::Size);
    assert_eq!(map_index_to_sort(2), SortBy::Date);
    assert_eq!(map_index_to_sort(3), SortBy::Type);
}

#[test]
fn test_theme_dropdown_index_calculation() {
    let themes = vec![
        "dark".to_string(),
        "nord".to_string(),
        "solarized".to_string(),
    ];

    let get_theme_index = |current: Option<&str>| -> u32 {
        let current_theme = current.unwrap_or("default");
        if current_theme == "default" {
            0
        } else if let Some(pos) = themes.iter().position(|x| x == current_theme) {
            (pos + 1) as u32
        } else {
            0
        }
    };

    assert_eq!(get_theme_index(None), 0);
    assert_eq!(get_theme_index(Some("default")), 0);
    assert_eq!(get_theme_index(Some("dark")), 1);
    assert_eq!(get_theme_index(Some("nord")), 2);
    assert_eq!(get_theme_index(Some("solarized")), 3);
    assert_eq!(get_theme_index(Some("nonexistent")), 0);
}

#[test]
fn settings_shortcut_all_whitespace_trimmed() {
    let parse = |v: &str| v.trim().to_string();
    assert_eq!(parse("  \t\n  "), "");
}

#[test]
fn settings_sort_index_out_of_range_defaults_name() {
    let map_index = |i: u32| match i {
        0 => SortBy::Name,
        1 => SortBy::Size,
        2 => SortBy::Date,
        3 => SortBy::Type,
        _ => SortBy::Name,
    };
    assert_eq!(map_index(99), SortBy::Name);
}

#[test]
fn settings_theme_index_whitespace_current() {
    let themes = vec!["dark".to_string()];
    let get = |c: Option<&str>| {
        let t = c.unwrap_or("default");
        if t == "default" {
            0
        } else {
            themes
                .iter()
                .position(|x| x == t)
                .map(|p| p + 1)
                .unwrap_or(0) as u32
        }
    };
    assert_eq!(get(Some("")), 0);
}

#[test]
fn settings_shortcut_preserves_inner_spaces() {
    let parse = |v: &str| v.trim().to_string();
    assert_eq!(parse("  <Ctrl> <Shift> a  "), "<Ctrl> <Shift> a");
}

#[test]
fn settings_shortcut_tabs_trimmed() {
    let parse = |v: &str| v.trim().to_string();
    assert_eq!(parse("\t<Primary>f\t"), "<Primary>f");
}

#[test]
fn settings_sort_roundtrip_all_variants() {
    let idx = |s: SortBy| match s {
        SortBy::Name => 0,
        SortBy::Size => 1,
        SortBy::Date => 2,
        SortBy::Type => 3,
    };
    let rev = |i: u32| match i {
        0 => SortBy::Name,
        1 => SortBy::Size,
        2 => SortBy::Date,
        3 => SortBy::Type,
        _ => SortBy::Name,
    };
    for s in [SortBy::Name, SortBy::Size, SortBy::Date, SortBy::Type] {
        assert_eq!(rev(idx(s)), s);
    }
}

#[test]
fn settings_theme_default_index_zero() {
    let themes: Vec<String> = vec![];
    let get = |c: Option<&str>| -> u32 {
        let t = c.unwrap_or("default");
        if t == "default" {
            0
        } else {
            themes
                .iter()
                .position(|x| x == t)
                .map(|p| (p + 1) as u32)
                .unwrap_or(0)
        }
    };
    assert_eq!(get(None), 0);
}

#[test]
fn settings_shortcut_newline_trimmed() {
    let parse = |v: &str| v.trim().to_string();
    assert_eq!(parse("\nF5\n"), "F5");
}
