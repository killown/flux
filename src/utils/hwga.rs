//! Here We Go Again (HWGA) - lock-free call counter and telemetry probes for debug builds.
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::Once;
use std::time::Duration;
#[cfg(debug_assertions)]
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct FunctionStats {
    pub count: usize,
    pub total_time: Duration,
    pub min_time: Duration,
    pub max_time: Duration,
}

/// Stores atomic counters for a single hit point, initialized as a static via the hit macro.
pub struct Probe {
    name: &'static str,
    count: AtomicU64,
    total_ns: AtomicU64,
    min_ns: AtomicU64,
    max_ns: AtomicU64,
    registered: AtomicBool,
    next: AtomicPtr<Probe>,
}

static HEAD: AtomicPtr<Probe> = AtomicPtr::new(ptr::null_mut());
static ATEXIT: Once = Once::new();

impl Probe {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            count: AtomicU64::new(0),
            total_ns: AtomicU64::new(0),
            min_ns: AtomicU64::new(u64::MAX),
            max_ns: AtomicU64::new(0),
            registered: AtomicBool::new(false),
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// Increments call counts and updates timing statistics using lock-free relaxed atomics.
    #[inline]
    pub fn record(&'static self, elapsed: Duration) {
        if !self.registered.load(Ordering::Relaxed) {
            self.register();
        }
        let ns = elapsed.as_nanos().min(u64::MAX as u128) as u64;
        self.count.fetch_add(1, Ordering::Relaxed);
        self.total_ns.fetch_add(ns, Ordering::Relaxed);
        self.min_ns.fetch_min(ns, Ordering::Relaxed);
        self.max_ns.fetch_max(ns, Ordering::Relaxed);
    }

    /// Links this probe into the global linked list safely on its first hit.
    #[cold]
    fn register(&'static self) {
        if self.registered.swap(true, Ordering::AcqRel) {
            return;
        }
        ATEXIT.call_once(|| unsafe {
            libc::atexit(dump_extern);
        });
        let me = self as *const Probe as *mut Probe;
        let mut head = HEAD.load(Ordering::Acquire);
        loop {
            self.next.store(head, Ordering::Relaxed);
            match HEAD.compare_exchange_weak(head, me, Ordering::Release, Ordering::Acquire) {
                Ok(_) => break,
                Err(current) => head = current,
            }
        }
    }

    /// Clears the probe counters back to zero.
    fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        self.total_ns.store(0, Ordering::Relaxed);
        self.min_ns.store(u64::MAX, Ordering::Relaxed);
        self.max_ns.store(0, Ordering::Relaxed);
    }
}

struct ProbeIter(*mut Probe);

impl Iterator for ProbeIter {
    type Item = &'static Probe;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_null() {
            return None;
        }
        // Probes are static allocations that live forever, so the lifetime extension is safe.
        let probe: &'static Probe = unsafe { &*self.0 };
        self.0 = probe.next.load(Ordering::Acquire);
        Some(probe)
    }
}

fn probes() -> ProbeIter {
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

pub struct CallMonitor;

impl CallMonitor {
    /// Resets all probes.
    #[allow(dead_code)]
    pub fn reset() {
        reset_all();
    }

    /// Gathers cumulative statistics for a single label across all matching call sites.
    #[allow(dead_code)]
    pub fn get_stats(name: &str) -> Option<FunctionStats> {
        Self::snapshot()
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, s)| s)
    }

    /// Takes a point-in-time snapshot of all probes, merging matches by label and sorting by total time.
    pub fn snapshot() -> Vec<(&'static str, FunctionStats)> {
        let mut merged: HashMap<&'static str, FunctionStats> = HashMap::new();
        for probe in probes() {
            let calls = probe.count.load(Ordering::Relaxed);
            if calls == 0 {
                continue;
            }
            let total = Duration::from_nanos(probe.total_ns.load(Ordering::Relaxed));
            let min = Duration::from_nanos(probe.min_ns.load(Ordering::Relaxed));
            let max = Duration::from_nanos(probe.max_ns.load(Ordering::Relaxed));
            merged
                .entry(probe.name)
                .and_modify(|s| {
                    s.count += calls as usize;
                    s.total_time += total;
                    s.min_time = s.min_time.min(min);
                    s.max_time = s.max_time.max(max);
                })
                .or_insert(FunctionStats {
                    count: calls as usize,
                    total_time: total,
                    min_time: min,
                    max_time: max,
                });
        }
        let mut out: Vec<(&'static str, FunctionStats)> = merged.into_iter().collect();
        out.sort_by_key(|entry| std::cmp::Reverse(entry.1.total_time));
        out
    }

    /// Prints a formatted summary table of all telemetry stats to stdout.
    pub fn dump() {
        let stats = Self::snapshot();
        if stats.is_empty() {
            return;
        }
        println!("\n=== HWGA Call Frequency & Timing Report ===");
        for (name, stats) in stats {
            let avg_time = stats.total_time / stats.count.max(1) as u32;
            println!(
                "- {}: {} calls | Total: {:?} | Avg: {:?} | Min: {:?} | Max: {:?}",
                name, stats.count, stats.total_time, avg_time, stats.min_time, stats.max_time
            );
        }
        println!("===========================================\n");
    }

    /// Serializes active telemetry stats into a JSON file at the specified path.
    #[allow(dead_code)]
    pub fn dump_json_to<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            let _ = fs::create_dir_all(parent);
        }

        let stats = Self::snapshot();
        let mut out = String::from("{\n");
        out.push_str(&format!("  \"pid\": {},\n", std::process::id()));
        out.push_str("  \"functions\": {\n");

        let mut iter = stats.iter().peekable();
        while let Some((name, stats)) = iter.next() {
            let avg_micros = stats.total_time.as_micros() / stats.count.max(1) as u128;
            out.push_str(&format!(
                "    \"{}\": {{\n      \"calls\": {},\n      \"total_micros\": {},\n      \"avg_micros\": {},\n      \"min_micros\": {},\n      \"max_micros\": {}\n    }}",
                name,
                stats.count,
                stats.total_time.as_micros(),
                avg_micros,
                stats.min_micros(),
                stats.max_time.as_micros()
            ));
            if iter.peek().is_some() {
                out.push(',');
            }
            out.push('\n');
        }

        out.push_str("  }\n}\n");
        fs::write(path, out)
    }
}

extern "C" fn dump_extern() {
    let Ok(target) = std::env::var("FLUX_HWGA_OUT") else {
        return;
    };

    CallMonitor::dump();

    let path = if target == "auto" || target == "1" {
        std::env::temp_dir()
            .join("flux-hwga")
            .join(format!("telemetry-{}.json", std::process::id()))
    } else {
        PathBuf::from(target)
    };

    let _ = CallMonitor::dump_json_to(&path);
}

/// Measures scope duration and flushes it to the probe when dropped.
pub struct CallGuard {
    #[cfg(debug_assertions)]
    probe: &'static Probe,
    #[cfg(debug_assertions)]
    start: Instant,
}

impl CallGuard {
    #[inline]
    pub fn new(probe: &'static Probe) -> Self {
        #[cfg(debug_assertions)]
        {
            Self {
                probe,
                start: Instant::now(),
            }
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = probe;
            Self {}
        }
    }
}

impl Drop for CallGuard {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            self.probe.record(self.start.elapsed());
        }
    }
}

/// Inserts a lock-free telemetry probe into the current scope.
///
/// - `hit!()` uses `module_path!()::line!()` automatically.
/// - `hit!("custom_name")` uses a stable name that won't shift if lines above it move.
///
/// Both compile down to absolute no-ops in release builds.
#[macro_export]
macro_rules! hit {
    () => {
        $crate::hit!(@probe concat!(module_path!(), "::", line!()));
    };
    ($name:literal) => {$crate::hit!(@probe concat!(module_path!(), "::", $name));
    };
    (@probe $full:expr) => {
        #[cfg(debug_assertions)]
        let _hwga_guard = {
            static PROBE: $crate::utils::hwga::Probe =$crate::utils::hwga::Probe::new($full);$crate::utils::hwga::CallGuard::new(&PROBE)
        };
    };
}
