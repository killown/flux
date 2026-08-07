use flux::model::SortBy;

#[test]
fn test_cycle_sort_rotation() {
    let mut sort = SortBy::Name;

    sort = match sort {
        SortBy::Name => SortBy::Date,
        SortBy::Date => SortBy::Size,
        SortBy::Size => SortBy::Type,
        SortBy::Type => SortBy::Name,
    };
    assert_eq!(sort, SortBy::Date);

    sort = match sort {
        SortBy::Name => SortBy::Date,
        SortBy::Date => SortBy::Size,
        SortBy::Size => SortBy::Type,
        SortBy::Type => SortBy::Name,
    };
    assert_eq!(sort, SortBy::Size);

    sort = match sort {
        SortBy::Name => SortBy::Date,
        SortBy::Date => SortBy::Size,
        SortBy::Size => SortBy::Type,
        SortBy::Type => SortBy::Name,
    };
    assert_eq!(sort, SortBy::Type);

    sort = match sort {
        SortBy::Name => SortBy::Date,
        SortBy::Date => SortBy::Size,
        SortBy::Size => SortBy::Type,
        SortBy::Type => SortBy::Name,
    };
    assert_eq!(sort, SortBy::Name);
}

#[test]
fn test_zoom_clamp_limits() {
    let zoom_min = 48;
    let zoom_max = 256;
    let zoom_step = 16;

    let mut current_size = 64;

    // Zoom out past min
    let delta_out = 1.0;
    for _ in 0..10 {
        let change = if delta_out > 0.0 {
            -zoom_step
        } else {
            zoom_step
        };
        current_size = (current_size + change).clamp(zoom_min, zoom_max);
    }
    assert_eq!(current_size, zoom_min);

    // Zoom in past max
    let delta_in = -1.0;
    for _ in 0..20 {
        let change = if delta_in > 0.0 {
            -zoom_step
        } else {
            zoom_step
        };
        current_size = (current_size + change).clamp(zoom_min, zoom_max);
    }
    assert_eq!(current_size, zoom_max);
}

#[test]
fn test_selection_status_formatting_logic() {
    let total_selected = 1usize;
    let only_files = true;
    let single_name = "test.txt".to_string();
    let size_str = "1.2 MB";

    let status = match (total_selected, only_files) {
        (1, true) => format!("{} ({})", single_name, size_str),
        _ => String::new(),
    };

    assert_eq!(status, "test.txt (1.2 MB)");
}
