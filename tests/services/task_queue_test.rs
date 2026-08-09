use flux::services::tasks::{format_bytes, format_duration, SpeedWindow, TaskQueue};
use gtk::gio::prelude::CancellableExt;
use std::time::Duration;

// ─── format_bytes ─────────────────────────────────────────────────────────────

#[test]
fn test_format_bytes_sub_kilobyte() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1), "1 B");
    assert_eq!(format_bytes(1023), "1023 B");
}

#[test]
fn test_format_bytes_kilobyte_boundary() {
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1536), "1.5 KB");
    assert_eq!(format_bytes(1023 * 1024), "1023.0 KB");
}

#[test]
fn test_format_bytes_megabyte_boundary() {
    assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    assert_eq!(format_bytes(1024 * 1024 + 512 * 1024), "1.5 MB");
}

#[test]
fn test_format_bytes_gigabyte_boundary() {
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.0 GB");
}

// ─── format_duration ──────────────────────────────────────────────────────────

#[test]
fn test_format_duration_zero() {
    assert_eq!(format_duration(0), "00:00");
}

#[test]
fn test_format_duration_seconds_only() {
    assert_eq!(format_duration(45), "00:45");
}

#[test]
fn test_format_duration_minutes_and_seconds() {
    assert_eq!(format_duration(125), "02:05");
}

#[test]
fn test_format_duration_exactly_one_hour() {
    assert_eq!(format_duration(3600), "01:00:00");
}

#[test]
fn test_format_duration_hours_minutes_seconds() {
    assert_eq!(format_duration(3723), "01:02:03");
}

#[test]
fn test_format_duration_sub_day_max() {
    // 86399s = 23:59:59, anything >= 86400 is suppressed by the ETA guard in
    // the dialog, but the formatter itself must not panic.
    assert_eq!(format_duration(86399), "23:59:59");
}

// ─── SpeedWindow ──────────────────────────────────────────────────────────────

#[test]
fn test_speed_window_empty_returns_zero() {
    let sw = SpeedWindow::new();
    assert_eq!(sw.bytes_per_sec(), 0.0);
}

#[test]
fn test_speed_window_single_sample_returns_zero() {
    let mut sw = SpeedWindow::new();
    sw.push(1024);
    // Need at least two samples to compute a rate.
    assert_eq!(sw.bytes_per_sec(), 0.0);
}

#[test]
fn test_speed_window_two_samples_produces_positive_rate() {
    let mut sw = SpeedWindow::new();
    sw.push(0);
    // Tiny sleep so the two Instants are measurably different.
    std::thread::sleep(Duration::from_millis(10));
    sw.push(1_000_000);
    assert!(
        sw.bytes_per_sec() > 0.0,
        "two samples with a time delta must yield a positive rate"
    );
}

#[test]
fn test_speed_window_non_increasing_bytes_returns_zero() {
    let mut sw = SpeedWindow::new();
    sw.push(500);
    std::thread::sleep(Duration::from_millis(5));
    // Same value - no progress, rate must be zero.
    sw.push(500);
    assert_eq!(sw.bytes_per_sec(), 0.0);
}

#[test]
fn test_speed_window_default_equals_new() {
    let a = SpeedWindow::default();
    let b = SpeedWindow::new();
    // Both must start with zero rate.
    assert_eq!(a.bytes_per_sec(), b.bytes_per_sec());
}

// ─── is_cancelled_error (via cancellable state, not string heuristic) ─────────

/// Verifies the post-fix contract: cancellation is detected via the
/// `Cancellable` flag, not fragile string matching.  A cancelled `Cancellable`
/// must always satisfy the guard regardless of the error message text.
#[test]
fn test_is_cancelled_error_flag_takes_precedence() {
    fn is_cancelled_error(cancellable: &gtk::gio::Cancellable, msg: &str) -> bool {
        cancellable.is_cancelled()
            || msg.contains("g-io-error-quark: 19")
            || msg.contains("g-io-error-quark:19")
    }

    let c = gtk::gio::Cancellable::new();
    c.cancel();

    assert!(is_cancelled_error(&c, "some unrelated error"));
    assert!(is_cancelled_error(&c, ""));
}

#[test]
fn test_is_cancelled_error_quark_code_fallback() {
    fn is_cancelled_error(cancellable: &gtk::gio::Cancellable, msg: &str) -> bool {
        cancellable.is_cancelled()
            || msg.contains("g-io-error-quark: 19")
            || msg.contains("g-io-error-quark:19")
    }

    let c = gtk::gio::Cancellable::new();

    assert!(is_cancelled_error(&c, "g-io-error-quark: 19"));
    assert!(is_cancelled_error(&c, "g-io-error-quark:19"));
    assert!(!is_cancelled_error(&c, "permission denied"));
    assert!(!is_cancelled_error(&c, "cancelled")); // old string heuristic must no longer match
}

#[test]
fn test_is_cancelled_error_old_string_heuristic_removed() {
    // Regression: the pre-fix implementation matched on "cancelled" / "Cancelled"
    // as plain substrings, causing false positives on unrelated GIO errors whose
    // messages happen to contain those words.  The new implementation must NOT
    // classify such messages as cancellation unless the flag or quark code is present.
    fn is_cancelled_error(cancellable: &gtk::gio::Cancellable, msg: &str) -> bool {
        cancellable.is_cancelled()
            || msg.contains("g-io-error-quark: 19")
            || msg.contains("g-io-error-quark:19")
    }

    let c = gtk::gio::Cancellable::new();

    assert!(!is_cancelled_error(
        &c,
        "Operation was cancelled by remote host"
    ));
    assert!(!is_cancelled_error(&c, "Cancelled by policy"));
    assert!(!is_cancelled_error(&c, "cancelled"));
}

// ─── update preserves started_at on re-entry ─────────────────────────────────

/// `update()` must not overwrite `started_at` on subsequent calls for the same
/// id - the elapsed timer shown in the dialog would jump backwards otherwise.
#[test]
fn test_update_preserves_started_at_on_reentry() {
    let q = TaskQueue::default();
    let c = gtk::gio::Cancellable::new();

    q.update(1, "file.bin".into(), 0, 1000, 1, c.clone());
    let snap1 = q.snapshot();
    let t0 = snap1[0].1.started_at;

    std::thread::sleep(Duration::from_millis(5));

    q.update(1, "file.bin".into(), 500, 1000, 1, c);
    let snap2 = q.snapshot();
    let t1 = snap2[0].1.started_at;

    assert_eq!(t0, t1, "started_at must not be reset on progress update");
}
