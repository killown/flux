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
