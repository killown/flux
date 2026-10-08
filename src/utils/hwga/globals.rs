use super::probe::Probe;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Once, OnceLock};
use std::time::{Duration, Instant};

/// Main-thread calls longer than this count as over budget (one 60 Hz frame).
pub const MAIN_THREAD_BUDGET: Duration = Duration::from_millis(16);
pub(super) const MAIN_THREAD_BUDGET_NS: u64 = 16_000_000;
pub(super) const BUCKETS: usize = 64;

pub(super) static HEAD: AtomicPtr<Probe> = AtomicPtr::new(ptr::null_mut());
pub(super) static ATEXIT: Once = Once::new();
pub(super) static START: OnceLock<Instant> = OnceLock::new();

struct ProbeIter(*mut Probe);

impl Iterator for ProbeIter {
    type Item = &'static Probe;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_null() {
            return None;
        }
        let probe: &'static Probe = unsafe { &*self.0 };
        self.0 = probe.next.load(Ordering::Acquire);
        Some(probe)
    }
}

pub(super) fn probes() -> impl Iterator<Item = &'static Probe> {
    ProbeIter(HEAD.load(Ordering::Acquire))
}

/// Returns the total invocation count for a specific probe label, or zero if it hasn't fired.
#[allow(dead_code)]
pub fn count(name: &str) -> u64 {
    probes()
        .filter(|p| p.name == name)
        .map(|p| p.count.load(Ordering::Relaxed))
        .sum()
}

/// Resets all registered probes, helpful at the beginning of unit or integration tests.
#[allow(dead_code)]
pub fn reset_all() {
    probes().for_each(Probe::reset);
}

pub(super) fn to_ns(d: Duration) -> u64 {
    d.as_nanos().min(u64::MAX as u128) as u64
}

/// Formats a duration with a unit that fits the magnitude.
pub(super) fn fmt_duration(d: Duration) -> String {
    let ns = d.as_nanos();
    if ns < 1_000 {
        format!("{}ns", ns)
    } else if ns < 1_000_000 {
        format!("{:.1}µs", ns as f64 / 1_000.0)
    } else if ns < 1_000_000_000 {
        format!("{:.2}ms", ns as f64 / 1_000_000.0)
    } else {
        format!("{:.2}s", ns as f64 / 1_000_000_000.0)
    }
}

/// Index of the log2 bucket holding `ns` (bucket `i` covers `2^i ..= 2^(i+1) - 1`).
pub(super) fn bucket_of(ns: u64) -> usize {
    (63 - ns.max(1).leading_zeros()) as usize
}

pub(super) fn percentile_ns(
    hist: &[u64; BUCKETS],
    count: u64,
    min_ns: u64,
    max_ns: u64,
    q: f64,
) -> u64 {
    let target = ((count as f64) * q).ceil().max(1.0) as u64;
    let mut seen = 0;
    for (i, n) in hist.iter().enumerate() {
        seen += n;
        if seen >= target {
            let upper = if i >= 63 {
                u64::MAX
            } else {
                (1u64 << (i + 1)) - 1
            };
            return upper.clamp(min_ns, max_ns);
        }
    }
    max_ns
}
