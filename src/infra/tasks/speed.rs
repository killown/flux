use std::time::Instant;

/// Maximum age of a speed sample (seconds). Samples older than this are dropped.
const SPEED_WINDOW_SECS: f64 = 2.0;
/// Maximum number of samples retained (ring buffer cap).
const SPEED_RING_CAP: usize = 64;

/// Fixed-capacity ring buffer that computes a smoothed transfer rate.
///
/// Only samples within the last [`SPEED_WINDOW_SECS`] contribute to the
/// average, so the rate adapts quickly without the "999 hours remaining"
/// instability of a purely instantaneous delta.
#[derive(Debug, Clone)]
pub struct SpeedWindow {
    /// Circular buffer of `(timestamp, bytes_transferred_at_that_point)`.
    samples: Vec<(Instant, u64)>,
    head: usize,
    len: usize,
}

impl Default for SpeedWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeedWindow {
    pub fn new() -> Self {
        Self {
            samples: Vec::with_capacity(SPEED_RING_CAP),
            head: 0,
            len: 0,
        }
    }

    /// Record a new cumulative byte count.
    pub fn push(&mut self, bytes: u64) {
        let now = Instant::now();
        if self.samples.len() < SPEED_RING_CAP {
            self.samples.push((now, bytes));
            self.len += 1;
        } else {
            self.samples[self.head] = (now, bytes);
            self.head = (self.head + 1) % SPEED_RING_CAP;
        }
    }

    /// Returns the smoothed transfer rate in **bytes per second**.
    ///
    /// Uses all samples within the last [`SPEED_WINDOW_SECS`] seconds.
    /// Returns `0.0` if fewer than two qualifying samples exist.
    pub fn bytes_per_sec(&self) -> f64 {
        if self.len < 2 {
            return 0.0;
        }

        let now = Instant::now();
        let cutoff = SPEED_WINDOW_SECS;

        // Collect samples within the window (in insertion order).
        let mut window: Vec<(Instant, u64)> = self
            .samples
            .iter()
            .copied()
            .filter(|(t, _)| now.duration_since(*t).as_secs_f64() <= cutoff)
            .collect();

        window.sort_by_key(|(t, _)| *t);

        if window.len() < 2 {
            return 0.0;
        }

        let (t0, b0) = window[0];
        let (t1, b1) = window[window.len() - 1];
        let dt = t1.duration_since(t0).as_secs_f64();

        if dt < 1e-6 || b1 <= b0 {
            return 0.0;
        }

        (b1 - b0) as f64 / dt
    }
}
