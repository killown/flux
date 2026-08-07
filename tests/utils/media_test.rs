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
    assert_eq!(aspect_ratio_label(700, 300), "7:3");
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
