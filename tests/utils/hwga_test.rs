#![cfg(debug_assertions)]

use flux::hwga::{self, CallMonitor};
use std::sync::Mutex;

static SERIAL: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn label(name: &str) -> String {
    format!("{}::{}", module_path!(), name)
}

fn counted_call() {
    flux::hit!("counted_call");
}

#[test]
fn counts_each_call() {
    let _g = lock();
    hwga::reset_all();
    for _ in 0..25 {
        counted_call();
    }
    assert_eq!(hwga::count(&label("counted_call")), 25);
}

#[test]
fn unknown_label_is_zero() {
    let _g = lock();
    assert_eq!(hwga::count(&label("does_not_exist")), 0);
}

#[test]
fn reset_zeroes_counts() {
    let _g = lock();
    counted_call();
    hwga::reset_all();
    assert_eq!(hwga::count(&label("counted_call")), 0);
    assert!(CallMonitor::get_stats(&label("counted_call")).is_none());
}

#[test]
fn sites_with_same_label_are_merged() {
    let _g = lock();
    hwga::reset_all();
    {
        flux::hit!("shared_label");
    }
    {
        flux::hit!("shared_label");
    }
    assert_eq!(hwga::count(&label("shared_label")), 2);
    let stats = CallMonitor::get_stats(&label("shared_label")).unwrap();
    assert_eq!(stats.count, 2);
}

#[test]
fn line_labelled_probe_uses_module_and_line() {
    let _g = lock();
    hwga::reset_all();
    let line = line!() + 2;
    {
        flux::hit!();
    }
    assert_eq!(hwga::count(&label(&line.to_string())), 1);
}

#[test]
fn records_elapsed_time() {
    let _g = lock();
    hwga::reset_all();
    {
        flux::hit!("sleepy");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let stats = CallMonitor::get_stats(&label("sleepy")).unwrap();
    assert!(stats.min_time >= std::time::Duration::from_millis(5));
    assert!(stats.max_time >= stats.min_time);
    assert!(stats.total_time >= stats.max_time);
}

#[test]
fn concurrent_hits_are_not_lost() {
    let _g = lock();
    hwga::reset_all();
    let threads: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                for _ in 0..10_000 {
                    counted_call();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(hwga::count(&label("counted_call")), 80_000);
}

#[test]
fn json_dump_lists_recorded_probes() {
    let _g = lock();
    hwga::reset_all();
    counted_call();
    let path = std::env::temp_dir().join(format!("hwga-test-{}.json", std::process::id()));
    CallMonitor::dump_json_to(&path).unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(body.contains(&format!("\"{}\"", label("counted_call"))));
    assert!(body.contains("\"calls\": 1"));
}

#[test]
fn percentiles_separate_the_tail_from_the_typical_call() {
    static PROBE: flux::hwga::Probe = flux::hwga::Probe::new("hwga_test::pct_probe");
    let _g = lock();
    hwga::reset_all();
    for _ in 0..90 {
        PROBE.record(std::time::Duration::from_micros(1));
    }
    for _ in 0..10 {
        PROBE.record(std::time::Duration::from_millis(10));
    }
    let s = CallMonitor::get_stats("hwga_test::pct_probe").unwrap();
    assert!(
        s.p50 <= std::time::Duration::from_micros(4),
        "p50 {:?}",
        s.p50
    );
    assert!(
        s.p95 >= std::time::Duration::from_millis(8),
        "p95 {:?}",
        s.p95
    );
    assert!(s.p50 <= s.p95 && s.p95 <= s.p99 && s.p99 <= s.max_time);
}

#[test]
fn self_time_excludes_nested_probes() {
    let _g = lock();
    hwga::reset_all();
    {
        flux::hit!("outer");
        std::thread::sleep(std::time::Duration::from_millis(5));
        {
            flux::hit!("inner");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    let outer = CallMonitor::get_stats(&label("outer")).unwrap();
    let inner = CallMonitor::get_stats(&label("inner")).unwrap();
    assert!(outer.total_time >= std::time::Duration::from_millis(25));
    assert!(
        outer.self_time < outer.total_time - std::time::Duration::from_millis(15),
        "outer self {:?} total {:?}",
        outer.self_time,
        outer.total_time
    );
    assert_eq!(inner.self_time, inner.total_time);
}

#[test]
fn main_thread_budget_is_tracked_separately() {
    static PROBE: flux::hwga::Probe = flux::hwga::Probe::new("hwga_test::main_probe");
    let _g = lock();
    hwga::reset_all();
    let fast = std::time::Duration::from_millis(1);
    let slow = std::time::Duration::from_millis(40);
    PROBE.record_full(fast, fast, true);
    PROBE.record_full(slow, slow, true);
    PROBE.record_full(slow, slow, false);
    let s = CallMonitor::get_stats("hwga_test::main_probe").unwrap();
    assert_eq!(s.count, 3);
    assert_eq!(s.main_count, 2);
    assert_eq!(s.main_over_budget, 1);
    assert_eq!(s.main_max, slow);
}

#[test]
fn test_threads_are_not_the_main_thread() {
    let _g = lock();
    hwga::reset_all();
    let worker = std::thread::spawn(counted_call);
    worker.join().unwrap();
    let s = CallMonitor::get_stats(&label("counted_call")).unwrap();
    assert_eq!(s.main_count, 0);
}

#[test]
fn peak_in_flight_sees_simultaneous_calls() {
    let _g = lock();
    hwga::reset_all();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                flux::hit!("overlap");
                barrier.wait();
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let s = CallMonitor::get_stats(&label("overlap")).unwrap();
    assert_eq!(s.peak_in_flight, 4);
}

#[test]
fn json_has_header_and_new_fields() {
    let _g = lock();
    hwga::reset_all();
    counted_call();
    let path = std::env::temp_dir().join(format!("hwga-test-new-{}.json", std::process::id()));
    CallMonitor::dump_json_to(&path).unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    for key in [
        "\"version\"",
        "\"debug_assertions\"",
        "\"self_micros\"",
        "\"p95_nanos\"",
        "\"main_over_budget\"",
        "\"peak_in_flight\"",
    ] {
        assert!(body.contains(key), "missing {key} in {body}");
    }
}

#[test]
fn multi_level_nesting_self_time() {
    let _g = lock();
    hwga::reset_all();
    {
        flux::hit!("root");
        std::thread::sleep(std::time::Duration::from_millis(2));
        {
            flux::hit!("mid");
            std::thread::sleep(std::time::Duration::from_millis(5));
            {
                flux::hit!("leaf");
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
    let root = CallMonitor::get_stats(&label("root")).unwrap();
    let mid = CallMonitor::get_stats(&label("mid")).unwrap();
    let leaf = CallMonitor::get_stats(&label("leaf")).unwrap();

    assert!(root.self_time < root.total_time);
    assert!(mid.self_time < mid.total_time);
    assert_eq!(leaf.self_time, leaf.total_time);
}

#[test]
fn reset_clears_active_snapshots() {
    let _g = lock();
    hwga::reset_all();
    counted_call();
    assert!(!CallMonitor::snapshot().is_empty());

    CallMonitor::reset();
    assert!(CallMonitor::snapshot().is_empty());
}

#[test]
fn concurrent_probe_recording_stress() {
    static PROBE: flux::hwga::Probe = flux::hwga::Probe::new("hwga_test::stress_probe");
    let _g = lock();
    hwga::reset_all();

    let threads: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                for _ in 0..1000 {
                    PROBE.record(std::time::Duration::from_micros(50));
                }
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }

    let stats = CallMonitor::get_stats("hwga_test::stress_probe").unwrap();
    assert_eq!(stats.count, 8000);
}

#[test]
fn zero_and_tiny_durations_are_safe() {
    static PROBE: flux::hwga::Probe = flux::hwga::Probe::new("hwga_test::zero_probe");
    let _g = lock();
    hwga::reset_all();

    PROBE.record(std::time::Duration::ZERO);
    PROBE.record(std::time::Duration::from_nanos(1));

    let stats = CallMonitor::get_stats("hwga_test::zero_probe").unwrap();
    assert_eq!(stats.count, 2);
    assert_eq!(stats.min_time, std::time::Duration::ZERO);
}

#[test]
fn hotpath_config_load_is_bounded() {
    const PROBE: &str = "flux::utils::config::load_config";
    let _g = lock();
    let sandbox = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", sandbox.path());
    std::env::set_var("XDG_CONFIG_HOME", sandbox.path().join("config"));
    std::env::set_var("XDG_DATA_HOME", sandbox.path().join("data"));
    hwga::reset_all();

    let _ = flux::utils::load_config();
    let baseline = hwga::count(PROBE);

    for i in 0..500 {
        let _ = flux::services::loader::get_extension_icon_path(&format!("flx{}", i % 5));
    }

    let hits = hwga::count(PROBE) - baseline;
    assert!(
        hits <= 10,
        "load_config fired {hits}x across 500 icon lookups"
    );
}
