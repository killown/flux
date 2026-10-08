use super::globals::{
    count as _count, fmt_duration, percentile_ns, probes, reset_all, BUCKETS, MAIN_THREAD_BUDGET,
    START,
};
use super::stats::FunctionStats;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

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

// Silence the "unused import" for `count` - it's re-exported by `mod.rs`
// but this file doesn't call it directly.
#[allow(unused_imports)]
use _count as _;

/// The `atexit` handler: dumps to stderr and optionally writes a JSON file
/// when `FLUX_HWGA_OUT` is set.
pub(super) extern "C" fn dump_extern() {
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
