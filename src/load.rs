//! Run-queue load averages from `/proc/loadavg`.

use std::{
   fs,
   io,
   num::NonZeroUsize,
   thread,
};

/// An error reading or parsing `/proc/loadavg`.
#[derive(Debug, flub::Error)]
pub enum Error {
   /// `/proc/loadavg` is absent, as on a non-Linux host.
   #[error("load averages are unavailable on this system")]
   Unsupported,
   /// Reading `/proc/loadavg` failed for a reason other than its absence.
   #[error("reading /proc/loadavg")]
   Io(#[source] io::Error),
   /// The contents lacked a load average or the `running/total` pair.
   #[error("malformed /proc/loadavg")]
   Malformed,
}

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
   /// Returns [`Error::Unsupported`] when the file is absent, and
   /// [`Error::Io`] or [`Error::Malformed`] on other read or parse failures.
   #[inline]
   pub fn read() -> Result<Self, Error> {
      let text = match fs::read_to_string("/proc/loadavg") {
         Ok(text) => text,
         Err(err) if err.kind() == io::ErrorKind::NotFound => return Err(Error::Unsupported),
         Err(err) => return Err(Error::Io(err)),
      };
      parse_loadavg(&text)
   }

   /// The highest of the three load averages normalized per logical CPU.
   ///
   /// This is the worst-case saturation over the sampling windows: `1.0` means
   /// the run queue, at its busiest window, held one runnable task per core.
   #[inline]
   #[must_use]
   #[expect(clippy::float_arithmetic, reason = "load averages are floating-point")]
   pub fn per_cpu_peak(&self) -> f64 {
      let peak = self.load1.max(self.load5).max(self.load15);
      peak / f64::from(u32::try_from(cpu_count()).unwrap_or(u32::MAX))
   }
}

/// Parse the contents of `/proc/loadavg`.
///
/// The expected layout is `load1 load5 load15 running/total last_pid`; the
/// trailing `last_pid` field is ignored.
///
/// # Errors
///
/// Returns [`Error::Malformed`] when the three load averages or the
/// `running/total` pair are missing or unparseable.
#[inline]
#[expect(clippy::similar_names, reason = "load1/load5/load15 mirror /proc")]
pub fn parse_loadavg(text: &str) -> Result<Load, Error> {
   let parse = || {
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
   };
   parse().ok_or(Error::Malformed)
}

/// The number of logical CPUs available to this process.
///
/// Reflects the effective parallelism (honouring cgroup and affinity limits
/// where the platform exposes them). Falls back to `1` when the count cannot be
/// determined, so the result is always usable as a divisor.
#[inline]
#[must_use]
pub fn cpu_count() -> usize {
   thread::available_parallelism().map_or(1, NonZeroUsize::get)
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn parses_canonical_line() {
      let load = parse_loadavg("0.52 0.58 0.59 2/1234 56789").expect("canonical line");
      assert!((load.load1 - 0.52).abs() < 1.0e-9_f64);
      assert!((load.load5 - 0.58).abs() < 1.0e-9_f64);
      assert!((load.load15 - 0.59).abs() < 1.0e-9_f64);
      assert_eq!(load.running, 2);
      assert_eq!(load.total, 1234);
   }

   #[test]
   fn tolerates_trailing_whitespace_and_missing_pid() {
      let load = parse_loadavg("1.00 2.00 3.00 1/10\n").expect("trailing newline");
      assert_eq!(load.total, 10);
   }

   #[test]
   fn rejects_missing_fields() {
      assert!(matches!(parse_loadavg("0.10 0.20"), Err(Error::Malformed)));
   }

   #[test]
   fn rejects_bad_proc_pair() {
      assert!(matches!(
         parse_loadavg("0.10 0.20 0.30 notapair 1"),
         Err(Error::Malformed)
      ));
   }

   #[test]
   fn cpu_count_is_positive() {
      assert!(cpu_count() >= 1);
   }

   #[test]
   fn per_cpu_peak_uses_largest_window() {
      let load = Load {
         load1:   1.0,
         load5:   8.0,
         load15:  2.0,
         running: 1,
         total:   1,
      };
      let cpus = f64::from(u32::try_from(cpu_count()).expect("CPU count fits in u32"));
      assert!((load.per_cpu_peak() - 8.0_f64 / cpus).abs() < 1.0e-9_f64);
   }
}
