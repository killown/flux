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
