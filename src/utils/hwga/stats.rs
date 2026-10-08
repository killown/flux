use std::time::Duration;

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
    /// Main-thread calls longer than [`crate::utils::hwga::MAIN_THREAD_BUDGET`].
    pub main_over_budget: usize,
    /// Most guards of this probe alive at once, across all threads.
    pub peak_in_flight: usize,
}
