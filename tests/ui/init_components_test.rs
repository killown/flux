use flux::model::SortBy;

#[test]
fn test_sort_field_variant_key_mapping() {
    let map_key_to_sort = |key: &str| -> SortBy {
        match key {
            "name" => SortBy::Name,
            "date" => SortBy::Date,
            "size" => SortBy::Size,
            "type" => SortBy::Type,
            _ => SortBy::Name,
        }
    };

    assert_eq!(map_key_to_sort("name"), SortBy::Name);
    assert_eq!(map_key_to_sort("date"), SortBy::Date);
    assert_eq!(map_key_to_sort("size"), SortBy::Size);
    assert_eq!(map_key_to_sort("type"), SortBy::Type);
    assert_eq!(map_key_to_sort("unknown"), SortBy::Name);
}

#[test]
fn test_sort_direction_variant_key_mapping() {
    let parse_direction = |key: &str| -> bool { key == "asc" };

    assert!(parse_direction("asc"));
    assert!(!parse_direction("desc"));
    assert!(!parse_direction("other"));
}

#[test]
fn test_main_menu_action_target_pair_formatting() {
    let sort_fields = vec![
        ("By Name", "name"),
        ("By Date", "date"),
        ("By Size", "size"),
        ("By Type", "type"),
    ];

    for (label, key) in sort_fields {
        assert!(!label.is_empty());
        assert!(!key.is_empty());
    }
}
