use flux::utils::media::{aspect_ratio_label, format_duration};
use std::time::Duration;

#[test]
fn test_aspect_ratio_known_16_9() {
    assert_eq!(aspect_ratio_label(1920, 1080), "16:9");
    assert_eq!(aspect_ratio_label(1280, 720), "16:9");
    assert_eq!(aspect_ratio_label(3840, 2160), "16:9");
}

#[test]
fn test_aspect_ratio_known_4_3() {
    assert_eq!(aspect_ratio_label(1024, 768), "4:3");
    assert_eq!(aspect_ratio_label(640, 480), "4:3");
}

#[test]
fn test_aspect_ratio_square() {
    assert_eq!(aspect_ratio_label(512, 512), "1:1");
    assert_eq!(aspect_ratio_label(1, 1), "1:1");
}

#[test]
fn test_aspect_ratio_portrait_9_16() {
    assert_eq!(aspect_ratio_label(1080, 1920), "9:16");
}

#[test]
fn test_aspect_ratio_portrait_2_3() {
    assert_eq!(aspect_ratio_label(2, 3), "2:3");
}

#[test]
fn test_aspect_ratio_non_standard_fallback() {
    assert_eq!(aspect_ratio_label(700, 400), "7:4");
}

#[test]
fn test_aspect_ratio_zero_width() {
    assert_eq!(aspect_ratio_label(0, 1080), "");
}

#[test]
fn test_aspect_ratio_zero_height() {
    assert_eq!(aspect_ratio_label(1920, 0), "");
}

#[test]
fn test_aspect_ratio_both_zero() {
    assert_eq!(aspect_ratio_label(0, 0), "");
}

#[test]
fn test_format_duration_seconds_only() {
    assert_eq!(format_duration(Duration::from_secs(45)), "0:45");
}

#[test]
fn test_format_duration_minutes_and_seconds() {
    assert_eq!(format_duration(Duration::from_secs(125)), "2:05");
}

#[test]
fn test_format_duration_exactly_one_hour() {
    assert_eq!(format_duration(Duration::from_secs(3600)), "1:00:00");
}

#[test]
fn test_format_duration_hours_minutes_seconds() {
    assert_eq!(format_duration(Duration::from_secs(3723)), "1:02:03");
}

#[test]
fn test_format_duration_zero() {
    assert_eq!(format_duration(Duration::from_secs(0)), "0:00");
}

#[test]
fn test_format_duration_sub_second_truncates() {
    assert_eq!(format_duration(Duration::from_millis(59_999)), "0:59");
}

#[test]
fn test_gcd_coprime_inputs_produce_unreduced_ratio() {
    assert_eq!(aspect_ratio_label(7, 9), "7:9");
}

#[test]
fn test_gcd_large_common_factor() {
    assert_eq!(aspect_ratio_label(1000, 1000), "1:1");
}

#[test]
fn aspect_ratio_21_9() {
    assert_eq!(aspect_ratio_label(2520, 1080), "21:9");
    assert_eq!(aspect_ratio_label(700, 300), "21:9");
}

#[test]
fn aspect_ratio_ultrawide_monitor_resolutions_are_not_exactly_21_9() {
    assert_eq!(aspect_ratio_label(2560, 1080), "64:27");
    assert_eq!(aspect_ratio_label(3440, 1440), "43:18");
}

#[test]
fn aspect_ratio_16_10() {
    assert_eq!(aspect_ratio_label(1920, 1200), "16:10");
    assert_eq!(aspect_ratio_label(1680, 1050), "16:10");
    assert_eq!(aspect_ratio_label(2560, 1600), "16:10");
    assert_eq!(aspect_ratio_label(16, 10), "16:10");
}

#[test]
fn aspect_ratio_named_ratios_survive_gcd_reduction() {
    for (w, h, label) in [
        (1600, 900, "16:9"),
        (1600, 1200, "4:3"),
        (1800, 1200, "3:2"),
        (1250, 1000, "5:4"),
        (900, 1600, "9:16"),
        (1200, 1800, "2:3"),
        (2100, 900, "21:9"),
        (1600, 1000, "16:10"),
        (800, 800, "1:1"),
    ] {
        assert_eq!(aspect_ratio_label(w, h), label, "{w}x{h}");
    }
}

#[test]
fn aspect_ratio_3_2() {
    assert_eq!(aspect_ratio_label(3000, 2000), "3:2");
}

#[test]
fn aspect_ratio_5_4() {
    assert_eq!(aspect_ratio_label(1280, 1024), "5:4");
}

#[test]
fn aspect_ratio_prime_dimensions() {
    assert_eq!(aspect_ratio_label(13, 17), "13:17");
}

#[test]
fn format_duration_59_seconds() {
    assert_eq!(format_duration(Duration::from_secs(59)), "0:59");
}

#[test]
fn format_duration_60_seconds() {
    assert_eq!(format_duration(Duration::from_secs(60)), "1:00");
}

#[test]
fn format_duration_one_day() {
    assert_eq!(format_duration(Duration::from_secs(86400)), "24:00:00");
}

#[test]
fn format_duration_large_hours() {
    assert_eq!(
        format_duration(Duration::from_secs(100 * 3600)),
        "100:00:00"
    );
}

#[test]
fn aspect_ratio_1024_1000_reduces() {
    assert_eq!(aspect_ratio_label(1024, 1000), "128:125");
}
