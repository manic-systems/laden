//! Run-queue load averages from `/proc/loadavg`.

use std::{io, num::NonZeroUsize, thread};

/// A snapshot of the system load averages and scheduler run-queue counts.
///
/// The three load figures are the kernel's 1-, 5-, and 15-minute exponentially
/// damped averages of the number of runnable and uninterruptible tasks. Divide
/// them by [`cpu_count`] for a per-core figure comparable across hosts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Load {
  /// The 1-minute load average.
  pub load1:   f64,
  /// The 5-minute load average.
  pub load5:   f64,
  /// The 15-minute load average.
  pub load15:  f64,
  /// Tasks currently runnable or running (the numerator of the `/proc`
  /// `running/total` field).
  pub running: u64,
  /// Total tasks and threads on the system (the denominator of that field).
  pub total:   u64,
}

impl Load {
  /// Read and parse `/proc/loadavg`.
  ///
  /// # Errors
  ///
  /// Returns the underlying [`io::Error`] when the file cannot be read, and an
  /// [`io::ErrorKind::InvalidData`] error when its contents do not parse.
  pub fn read() -> io::Result<Self> {
    let text = std::fs::read_to_string("/proc/loadavg")?;
    parse_loadavg(&text).ok_or_else(|| {
      io::Error::new(io::ErrorKind::InvalidData, "malformed /proc/loadavg")
    })
  }

  /// The highest of the three load averages normalized per logical CPU.
  ///
  /// This is the worst-case saturation over the sampling windows: `1.0` means
  /// the run queue, at its busiest window, held one runnable task per core.
  #[must_use]
  pub fn per_cpu_peak(&self) -> f64 {
    let peak = self.load1.max(self.load5).max(self.load15);
    #[allow(clippy::cast_precision_loss)]
    let cpus = cpu_count() as f64;
    peak / cpus
  }
}

/// Parse the contents of `/proc/loadavg`.
///
/// The expected layout is `load1 load5 load15 running/total last_pid`; the
/// trailing `last_pid` field is ignored. Returns `None` when the three load
/// averages or the `running/total` pair are missing or unparseable.
#[must_use]
#[allow(clippy::similar_names)]
pub fn parse_loadavg(text: &str) -> Option<Load> {
  let mut fields = text.split_whitespace();
  let load1 = fields.next()?.parse().ok()?;
  let load5 = fields.next()?.parse().ok()?;
  let load15 = fields.next()?.parse().ok()?;
  let (running, total) = fields.next()?.split_once('/')?;
  Some(Load {
    load1,
    load5,
    load15,
    running: running.parse().ok()?,
    total: total.parse().ok()?,
  })
}

/// The number of logical CPUs available to this process.
///
/// Reflects the effective parallelism (honouring cgroup and affinity limits
/// where the platform exposes them). Falls back to `1` when the count cannot be
/// determined, so the result is always usable as a divisor.
#[must_use]
pub fn cpu_count() -> usize {
  thread::available_parallelism().map_or(1, NonZeroUsize::get)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_canonical_line() {
    let load =
      parse_loadavg("0.52 0.58 0.59 2/1234 56789").expect("canonical line");
    assert!((load.load1 - 0.52).abs() < 1e-9);
    assert!((load.load5 - 0.58).abs() < 1e-9);
    assert!((load.load15 - 0.59).abs() < 1e-9);
    assert_eq!(load.running, 2);
    assert_eq!(load.total, 1234);
  }

  #[test]
  fn tolerates_trailing_whitespace_and_missing_pid() {
    let load =
      parse_loadavg("1.00 2.00 3.00 1/10\n").expect("trailing newline");
    assert_eq!(load.total, 10);
  }

  #[test]
  fn rejects_missing_fields() {
    assert!(parse_loadavg("0.10 0.20").is_none());
  }

  #[test]
  fn rejects_bad_proc_pair() {
    assert!(parse_loadavg("0.10 0.20 0.30 notapair 1").is_none());
  }

  #[test]
  fn cpu_count_is_positive() {
    assert!(cpu_count() >= 1);
  }

  #[test]
  fn per_cpu_peak_uses_largest_window() {
    let load = Load {
      load1: 1.0,
      load5: 8.0,
      load15: 2.0,
      running: 1,
      total: 1,
    };
    #[allow(clippy::cast_precision_loss)]
    let expected = 8.0 / cpu_count() as f64;
    assert!((load.per_cpu_peak() - expected).abs() < 1e-9);
  }
}
