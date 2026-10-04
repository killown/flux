//! Here We Go Again (HWGA) - lock-free call counter and telemetry probes for debug builds.
#![cfg_attr(not(debug_assertions), allow(dead_code))]
use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::{Once, OnceLock};
use std::time::{Duration, Instant};

/// Main-thread calls longer than this count as over budget (one 60 Hz frame).
pub const MAIN_THREAD_BUDGET: Duration = Duration::from_millis(16);
const MAIN_THREAD_BUDGET_NS: u64 = 16_000_000;
const BUCKETS: usize = 64;

#[derive(Clone, Copy, Debug)]
pub struct FunctionStats {
    pub count: usize,
    pub total_time: Duration,
    /// Total time minus time spent in nested probes on the same thread.
    pub self_time: Duration,
    pub min_time: Duration,
    pub max_time: Duration,
    /// Approximate percentiles from log2 buckets, clamped to the observed min/max.
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    /// Calls that ran on the process main thread (the GTK thread).
    pub main_count: usize,
    pub main_max: Duration,
    /// Main-thread calls longer than [`MAIN_THREAD_BUDGET`].
    pub main_over_budget: usize,
    /// Most guards of this probe alive at once, across all threads.
    pub peak_in_flight: usize,
}

#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicU64 = AtomicU64::new(0);

/// Stores atomic counters for a single hit point, initialized as a static via the hit macro.
pub struct Probe {
    name: &'static str,
    count: AtomicU64,
    total_ns: AtomicU64,
    self_ns: AtomicU64,
    min_ns: AtomicU64,
    max_ns: AtomicU64,
    main_count: AtomicU64,
    main_max_ns: AtomicU64,
    main_over_budget: AtomicU64,
    in_flight: AtomicU64,
    peak_in_flight: AtomicU64,
    hist: [AtomicU64; BUCKETS],
    registered: AtomicBool,
    next: AtomicPtr<Probe>,
}

static HEAD: AtomicPtr<Probe> = AtomicPtr::new(ptr::null_mut());
static ATEXIT: Once = Once::new();
static START: OnceLock<Instant> = OnceLock::new();

impl Probe {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            count: AtomicU64::new(0),
            total_ns: AtomicU64::new(0),
            self_ns: AtomicU64::new(0),
            min_ns: AtomicU64::new(u64::MAX),
            max_ns: AtomicU64::new(0),
            main_count: AtomicU64::new(0),
            main_max_ns: AtomicU64::new(0),
            main_over_budget: AtomicU64::new(0),
            in_flight: AtomicU64::new(0),
            peak_in_flight: AtomicU64::new(0),
            hist: [ZERO; BUCKETS],
            registered: AtomicBool::new(false),
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// Increments call counts and updates timing statistics using lock-free relaxed atomics.
    #[allow(dead_code)]
    #[inline]
    pub fn record(&'static self, elapsed: Duration) {
        self.record_full(elapsed, elapsed, false);
    }

    /// Like [`Probe::record`], with self time and whether the call ran on the main thread.
    #[inline]
    pub fn record_full(&'static self, elapsed: Duration, self_time: Duration, on_main: bool) {
        if !self.registered.load(Ordering::Relaxed) {
            self.register();
        }
        let ns = to_ns(elapsed);
        self.count.fetch_add(1, Ordering::Relaxed);
        self.total_ns.fetch_add(ns, Ordering::Relaxed);
        self.self_ns.fetch_add(to_ns(self_time), Ordering::Relaxed);
        self.min_ns.fetch_min(ns, Ordering::Relaxed);
        self.max_ns.fetch_max(ns, Ordering::Relaxed);
        self.hist[bucket_of(ns)].fetch_add(1, Ordering::Relaxed);
        if on_main {
            self.main_count.fetch_add(1, Ordering::Relaxed);
            self.main_max_ns.fetch_max(ns, Ordering::Relaxed);
            if ns > MAIN_THREAD_BUDGET_NS {
                self.main_over_budget.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    #[inline]
    fn enter(&self) {
        let now = self.in_flight.fetch_add(1, Ordering::Relaxed) + 1;
        self.peak_in_flight.fetch_max(now, Ordering::Relaxed);
    }

    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    #[inline]
    fn leave(&self) {
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
    }

    /// Links this probe into the global linked list safely on its first hit.
    #[cold]
    fn register(&'static self) {
        if self.registered.swap(true, Ordering::AcqRel) {
            return;
        }
        START.get_or_init(Instant::now);
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
        self.self_ns.store(0, Ordering::Relaxed);
        self.min_ns.store(u64::MAX, Ordering::Relaxed);
        self.max_ns.store(0, Ordering::Relaxed);
        self.main_count.store(0, Ordering::Relaxed);
        self.main_max_ns.store(0, Ordering::Relaxed);
        self.main_over_budget.store(0, Ordering::Relaxed);
        self.peak_in_flight
            .store(self.in_flight.load(Ordering::Relaxed), Ordering::Relaxed);
        self.hist.iter().for_each(|b| b.store(0, Ordering::Relaxed));
    }
}

fn to_ns(d: Duration) -> u64 {
    d.as_nanos().min(u64::MAX as u128) as u64
}

/// Formats a duration with a unit that fits the magnitude.
fn fmt_duration(d: Duration) -> String {
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
fn bucket_of(ns: u64) -> usize {
    (63 - ns.max(1).leading_zeros()) as usize
}

fn percentile_ns(hist: &[u64; BUCKETS], count: u64, min_ns: u64, max_ns: u64, q: f64) -> u64 {
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

#[derive(Default)]
struct Acc {
    count: u64,
    total_ns: u64,
    self_ns: u64,
    min_ns: u64,
    max_ns: u64,
    main_count: u64,
    main_max_ns: u64,
    main_over_budget: u64,
    peak_in_flight: u64,
    hist: Vec<u64>,
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
        let mut merged: HashMap<&'static str, Acc> = HashMap::new();
        for probe in probes() {
            let calls = probe.count.load(Ordering::Relaxed);
            if calls == 0 {
                continue;
            }
            let acc = merged.entry(probe.name).or_insert_with(|| Acc {
                min_ns: u64::MAX,
                hist: vec![0; BUCKETS],
                ..Acc::default()
            });
            acc.count += calls;
            acc.total_ns += probe.total_ns.load(Ordering::Relaxed);
            acc.self_ns += probe.self_ns.load(Ordering::Relaxed);
            acc.min_ns = acc.min_ns.min(probe.min_ns.load(Ordering::Relaxed));
            acc.max_ns = acc.max_ns.max(probe.max_ns.load(Ordering::Relaxed));
            acc.main_count += probe.main_count.load(Ordering::Relaxed);
            acc.main_max_ns = acc
                .main_max_ns
                .max(probe.main_max_ns.load(Ordering::Relaxed));
            acc.main_over_budget += probe.main_over_budget.load(Ordering::Relaxed);
            acc.peak_in_flight = acc
                .peak_in_flight
                .max(probe.peak_in_flight.load(Ordering::Relaxed));
            for (slot, bucket) in acc.hist.iter_mut().zip(probe.hist.iter()) {
                *slot += bucket.load(Ordering::Relaxed);
            }
        }

        let mut out: Vec<(&'static str, FunctionStats)> = merged
            .into_iter()
            .map(|(name, acc)| {
                let hist: [u64; BUCKETS] = acc.hist.clone().try_into().unwrap_or([0; BUCKETS]);
                let pct = |q| {
                    Duration::from_nanos(percentile_ns(&hist, acc.count, acc.min_ns, acc.max_ns, q))
                };
                (
                    name,
                    FunctionStats {
                        count: acc.count as usize,
                        total_time: Duration::from_nanos(acc.total_ns),
                        self_time: Duration::from_nanos(acc.self_ns),
                        min_time: Duration::from_nanos(acc.min_ns),
                        max_time: Duration::from_nanos(acc.max_ns),
                        p50: pct(0.50),
                        p95: pct(0.95),
                        p99: pct(0.99),
                        main_count: acc.main_count as usize,
                        main_max: Duration::from_nanos(acc.main_max_ns),
                        main_over_budget: acc.main_over_budget as usize,
                        peak_in_flight: acc.peak_in_flight as usize,
                    },
                )
            })
            .collect();
        out.sort_by_key(|entry| std::cmp::Reverse(entry.1.total_time));
        out
    }

    /// Prints a formatted summary table of all telemetry stats to stdout.
    pub fn dump() {
        let stats = Self::snapshot();
        if stats.is_empty() {
            return;
        }

        println!();
        println!("═══════════════════════════════════════════════════════════════════════════════");
        println!(
            "  HWGA Call Frequency & Timing Report - pid {}  flux {} {}",
            std::process::id(),
            env!("CARGO_PKG_VERSION"),
            option_env!("FLUX_GIT_HASH").unwrap_or(""),
        );
        println!(
            "  debug_assertions {}  |  up {:.1?} since first probe",
            cfg!(debug_assertions),
            START.get().map(|s| s.elapsed()).unwrap_or_default()
        );
        println!("═══════════════════════════════════════════════════════════════════════════════");
        println!();
        println!(
            "  {:>7}  {:>10}  {:>10}  {:>10}  {:>10}  {:>10}  {:>9}  Function",
            "Calls", "Total", "Self", "Avg", "p95", "Max", "Main>16ms"
        );
        println!(
            "  {:-<7}  {:-<10}  {:-<10}  {:-<10}  {:-<10}  {:-<10}  {:-<9}  {:-<40}",
            "", "", "", "", "", "", "", ""
        );

        let probe_count = stats.len();
        for (name, s) in stats {
            let avg = s.total_time / s.count.max(1) as u32;
            let main = if s.main_count > 0 {
                format!("{}/{}", s.main_over_budget, s.main_count)
            } else {
                "-".to_string()
            };

            println!(
                "  {:>7}  {:>10}  {:>10}  {:>10}  {:>10}  {:>10}  {:>9}  {}",
                s.count,
                fmt_duration(s.total_time),
                fmt_duration(s.self_time),
                fmt_duration(avg),
                fmt_duration(s.p95),
                fmt_duration(s.max_time),
                main,
                name,
            );
        }

        println!();
        println!(
            "  budget: {:?}/frame  |  {} probes",
            MAIN_THREAD_BUDGET, probe_count
        );
        println!("═══════════════════════════════════════════════════════════════════════════════");
        println!();
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
        out.push_str(&format!(
            "  \"version\": \"{}\",\n  \"git\": \"{}\",\n  \"debug_assertions\": {},\n  \"uptime_ms\": {},\n",
            env!("CARGO_PKG_VERSION"),
            option_env!("FLUX_GIT_HASH").unwrap_or(""),
            cfg!(debug_assertions),
            START.get().map(|s| s.elapsed().as_millis()).unwrap_or(0)
        ));
        out.push_str("  \"functions\": {\n");

        let mut iter = stats.iter().peekable();
        while let Some((name, s)) = iter.next() {
            let avg_micros = s.total_time.as_micros() / s.count.max(1) as u128;
            out.push_str(&format!(
                "    \"{}\": {{\n      \"calls\": {},\n      \"total_micros\": {},\n      \"self_micros\": {},\n      \"avg_micros\": {},\n      \"min_micros\": {},\n      \"max_micros\": {},\n      \"p50_nanos\": {},\n      \"p95_nanos\": {},\n      \"p99_nanos\": {},\n      \"main_calls\": {},\n      \"main_max_micros\": {},\n      \"main_over_budget\": {},\n      \"peak_in_flight\": {}\n    }}",
                name,
                s.count,
                s.total_time.as_micros(),
                s.self_time.as_micros(),
                avg_micros,
                s.min_time.as_micros(),
                s.max_time.as_micros(),
                s.p50.as_nanos(),
                s.p95.as_nanos(),
                s.p99.as_nanos(),
                s.main_count,
                s.main_max.as_micros(),
                s.main_over_budget,
                s.peak_in_flight
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

thread_local! {
    /// Time spent in nested probes by the guard currently open on this thread.
    static CHILD_NS: Cell<u64> = const { Cell::new(0) };
    static IS_MAIN: bool = is_main_thread();
}

#[cfg(target_os = "linux")]
#[cfg_attr(not(debug_assertions), allow(dead_code))]
fn is_main_thread() -> bool {
    unsafe { libc::syscall(libc::SYS_gettid) as libc::pid_t == libc::getpid() }
}

#[cfg(not(target_os = "linux"))]
#[cfg_attr(not(debug_assertions), allow(dead_code))]
fn is_main_thread() -> bool {
    false
}

/// Unique per live thread: the address of its `CHILD_NS` slot.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
fn thread_token() -> Option<usize> {
    CHILD_NS.try_with(|c| c as *const Cell<u64> as usize).ok()
}

/// Measures scope duration and flushes it to the probe when dropped.
pub struct CallGuard {
    #[cfg(debug_assertions)]
    probe: &'static Probe,
    #[cfg(debug_assertions)]
    parent_child_ns: u64,
    #[cfg(debug_assertions)]
    token: Option<usize>,
    #[cfg(debug_assertions)]
    on_main: bool,
    #[cfg(debug_assertions)]
    start: Instant,
}

impl CallGuard {
    #[inline]
    pub fn new(probe: &'static Probe) -> Self {
        #[cfg(debug_assertions)]
        {
            probe.enter();
            let parent_child_ns = CHILD_NS.try_with(|c| c.replace(0)).unwrap_or(0);
            let token = thread_token();
            let on_main = IS_MAIN.try_with(|m| *m).unwrap_or(false);
            Self {
                probe,
                parent_child_ns,
                token,
                on_main,
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
            let elapsed = self.start.elapsed();
            let elapsed_ns = to_ns(elapsed);
            let self_ns = match (self.token, thread_token()) {
                (Some(born), Some(now)) if born == now => CHILD_NS
                    .try_with(|c| {
                        let children = c.get();
                        c.set(self.parent_child_ns.saturating_add(elapsed_ns));
                        elapsed_ns.saturating_sub(children)
                    })
                    .unwrap_or(elapsed_ns),
                _ => elapsed_ns,
            };
            self.probe.leave();
            self.probe
                .record_full(elapsed, Duration::from_nanos(self_ns), self.on_main);
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
