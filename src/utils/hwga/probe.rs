use super::globals::{bucket_of, to_ns, ATEXIT, BUCKETS, HEAD, MAIN_THREAD_BUDGET_NS, START};
use super::monitor::dump_extern;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicU64 = AtomicU64::new(0);

/// Stores atomic counters for a single hit point, initialized as a static via the hit macro.
pub struct Probe {
    pub(super) name: &'static str,
    pub(super) count: AtomicU64,
    pub(super) total_ns: AtomicU64,
    pub(super) self_ns: AtomicU64,
    pub(super) min_ns: AtomicU64,
    pub(super) max_ns: AtomicU64,
    pub(super) main_count: AtomicU64,
    pub(super) main_max_ns: AtomicU64,
    pub(super) main_over_budget: AtomicU64,
    pub(super) in_flight: AtomicU64,
    pub(super) peak_in_flight: AtomicU64,
    pub(super) hist: [AtomicU64; BUCKETS],
    pub(super) registered: AtomicBool,
    pub(super) next: AtomicPtr<Probe>,
}

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

    #[inline]
    pub(super) fn enter(&self) {
        let now = self.in_flight.fetch_add(1, Ordering::Relaxed) + 1;
        self.peak_in_flight.fetch_max(now, Ordering::Relaxed);
    }

    #[inline]
    pub(super) fn leave(&self) {
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
    pub(super) fn reset(&self) {
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
