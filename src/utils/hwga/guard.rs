use super::globals::to_ns;
use super::probe::Probe;
use std::cell::Cell;
use std::time::{Duration, Instant};

thread_local! {
    /// Time spent in nested probes by the guard currently open on this thread.
    static CHILD_NS: Cell<u64> = const { Cell::new(0) };
    static IS_MAIN: bool = is_main_thread();
}

#[cfg(target_os = "linux")]
fn is_main_thread() -> bool {
    unsafe { libc::syscall(libc::SYS_gettid) as libc::pid_t == libc::getpid() }
}

#[cfg(not(target_os = "linux"))]
fn is_main_thread() -> bool {
    false
}

/// Unique per live thread: the address of its `CHILD_NS` slot.
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
