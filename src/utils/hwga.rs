/// Here We Go Again (HWGA) - Zero-cost execution telemetry and anti-regression call monitor.
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct FunctionStats {
    pub count: usize,
    pub total_time: Duration,
    pub min_time: Duration,
    pub max_time: Duration,
}

pub struct CallMonitor {
    stats: HashMap<&'static str, FunctionStats>,
}

impl CallMonitor {
    fn global() -> &'static Mutex<CallMonitor> {
        static MONITOR: std::sync::OnceLock<Mutex<CallMonitor>> = std::sync::OnceLock::new();
        MONITOR.get_or_init(|| {
            static INITIALIZED: AtomicBool = AtomicBool::new(false);
            if !INITIALIZED.swap(true, Ordering::SeqCst) {
                unsafe {
                    libc::atexit(dump_extern);
                }
            }
            Mutex::new(CallMonitor {
                stats: HashMap::new(),
            })
        })
    }

    pub fn record(name: &'static str, duration: Duration) {
        if let Ok(mut monitor) = Self::global().lock() {
            let entry = monitor.stats.entry(name).or_insert(FunctionStats {
                count: 0,
                total_time: Duration::ZERO,
                min_time: Duration::MAX,
                max_time: Duration::ZERO,
            });
            entry.count += 1;
            entry.total_time += duration;
            if duration < entry.min_time {
                entry.min_time = duration;
            }
            if duration > entry.max_time {
                entry.max_time = duration;
            }
        }
    }

    #[allow(dead_code)]
    pub fn reset() {
        if let Ok(mut monitor) = Self::global().lock() {
            monitor.stats.clear();
        }
    }

    #[allow(dead_code)]
    pub fn get_stats(name: &str) -> Option<FunctionStats> {
        if let Ok(monitor) = Self::global().lock() {
            monitor.stats.get(name).copied()
        } else {
            None
        }
    }

    /// Copy of all recorded stats, sorted by total time descending.
    pub fn snapshot() -> Vec<(&'static str, FunctionStats)> {
        let mut out: Vec<(&'static str, FunctionStats)> = Self::global()
            .lock()
            .map(|monitor| monitor.stats.iter().map(|(k, v)| (*k, *v)).collect())
            .unwrap_or_default();
        out.sort_by_key(|entry| std::cmp::Reverse(entry.1.total_time));
        out
    }

    pub fn dump() {
        if let Ok(monitor) = Self::global().lock() {
            if monitor.stats.is_empty() {
                return;
            }
            println!("\n=== HWGA Call Frequency & Timing Report ===");
            let mut sorted: Vec<(&&'static str, &FunctionStats)> = monitor.stats.iter().collect();
            sorted.sort_by_key(|a| std::cmp::Reverse(a.1.total_time));

            for (name, stats) in sorted {
                let avg_time = if stats.count > 0 {
                    stats.total_time / stats.count as u32
                } else {
                    Duration::ZERO
                };
                println!(
                    "- {}: {} calls | Total: {:?} | Avg: {:?} | Min: {:?} | Max: {:?}",
                    name, stats.count, stats.total_time, avg_time, stats.min_time, stats.max_time
                );
            }
            println!("===========================================\n");
        }
    }

    #[allow(dead_code)]
    pub fn dump_json_to<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
        if let Ok(monitor) = Self::global().lock() {
            if let Some(parent) = path.as_ref().parent() {
                let _ = fs::create_dir_all(parent);
            }

            let mut out = String::from("{\n");
            out.push_str(&format!("  \"pid\": {},\n", std::process::id()));
            out.push_str("  \"functions\": {\n");

            let mut iter = monitor.stats.iter().peekable();
            while let Some((name, stats)) = iter.next() {
                let avg_micros = if stats.count > 0 {
                    stats.total_time.as_micros() / stats.count as u128
                } else {
                    0
                };
                let min_micros = if stats.min_time == Duration::MAX {
                    0
                } else {
                    stats.min_time.as_micros()
                };

                out.push_str(&format!(
                    "    \"{}\": {{\n      \"calls\": {},\n      \"total_micros\": {},\n      \"avg_micros\": {},\n      \"min_micros\": {},\n      \"max_micros\": {}\n    }}",
                    name,
                    stats.count,
                    stats.total_time.as_micros(),
                    avg_micros,
                    min_micros,
                    stats.max_time.as_micros()
                ));

                if iter.peek().is_some() {
                    out.push(',');
                }
                out.push('\n');
            }

            out.push_str("  }\n}\n");
            fs::write(path, out)?;
        }
        Ok(())
    }
}

extern "C" fn dump_extern() {
    CallMonitor::dump();

    if let Ok(target) = std::env::var("FLUX_HWGA_OUT") {
        let path = if target == "auto" || target == "1" {
            std::env::temp_dir()
                .join("flux-hwga")
                .join(format!("telemetry-{}.json", std::process::id()))
        } else {
            PathBuf::from(target)
        };

        let _ = CallMonitor::dump_json_to(&path);
    }
}

pub struct CallGuard {
    #[cfg(debug_assertions)]
    name: &'static str,
    #[cfg(debug_assertions)]
    start: Instant,
}

impl CallGuard {
    #[inline]
    pub fn new(name: &'static str) -> Self {
        #[cfg(debug_assertions)]
        {
            Self {
                name,
                start: Instant::now(),
            }
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = name;
            Self
        }
    }
}

impl Drop for CallGuard {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            let elapsed = self.start.elapsed();
            CallMonitor::record(self.name, elapsed);
        }
    }
}

#[macro_export]
macro_rules! hit {
    () => {
        let _hwga_guard =
            $crate::utils::hwga::CallGuard::new(concat!(module_path!(), "::", line!()));
    };
}

#[macro_export]
macro_rules! hwga_fn {
    ($vis:vis fn $name:ident($($arg:tt)*) $(-> $ret:ty)? $body:block) => {
        $vis fn $name($($arg:tt)*) $(-> $ret)? {
            let _hwga_guard = $crate::utils::hwga::CallGuard::new(concat!(module_path!(), "::", stringify!($name)));
            $body
        }
    };
    ($vis:vis async fn $name:ident($($arg:tt)*) $(-> $ret:ty)? $body:block) => {
        $vis async fn $name($($arg:tt)*) $(-> $ret)? {
            let _hwga_guard = $crate::utils::hwga::CallGuard::new(concat!(module_path!(), "::", stringify!($name)));
            $body
        }
    };
}
