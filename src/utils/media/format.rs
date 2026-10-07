use std::time::Duration;

/// Returns a canonical aspect ratio label for a given resolution.
///
/// Reduces `width × height` by their GCD, then matches common display ratios.
/// Falls back to the reduced fraction string for non-standard ratios.
///
/// # Arguments
///
/// * `w` - Image width in pixels.
/// * `h` - Image height in pixels.
#[allow(dead_code)]
pub fn aspect_ratio_label(w: u32, h: u32) -> String {
    if w == 0 || h == 0 {
        return String::new();
    }

    let g = gcd(w, h);
    let rw = w / g;
    let rh = h / g;

    match (rw, rh) {
        (16, 9) => "16:9".into(),
        (4, 3) => "4:3".into(),
        (21, 9) => "21:9".into(),
        (1, 1) => "1:1".into(),
        (3, 2) => "3:2".into(),
        (5, 4) => "5:4".into(),
        (16, 10) => "16:10".into(),
        (9, 16) => "9:16".into(),
        (2, 3) => "2:3".into(),
        _ => format!("{}:{}", rw, rh),
    }
}

/// Formats a [`Duration`] into a human-readable `H:MM:SS` or `M:SS` string.
///
/// # Arguments
///
/// * `d` - The duration to format.
#[allow(dead_code)]
pub fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;

    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let rem = a % b;
        a = b;
        b = rem;
    }
    a
}
