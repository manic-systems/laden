//! PSI + rusage + EMA latency telemetry for load-aware schedulers.
//!
//! `pulse` gathers the signals a scheduler needs to place work on the
//! least-loaded host or agent:
//!
//! - [`psi`] — kernel pressure-stall information from
//!   `/proc/pressure/{cpu,memory,io}`.
//! - [`load`] — the run-queue load averages from `/proc/loadavg`, plus a
//!   [`cpu_count`] helper for normalizing them.
//! - [`rusage`] *(feature `rusage`)* — this process's peak resident set size.
//! - [`ema`] — an exponential moving average and a latency-driven [`Health`]
//!   tracker for smoothing upstream round-trip samples.
//!
//! The parsing entry points ([`parse_pressure`], [`parse_loadavg`]) are pure
//! functions over file contents, so they can be fed fixtures in tests without
//! touching `/proc`.
//!
//! # Example
//!
//! ```
//! use pulse::{Health, Status};
//!
//! let mut health = Health::new();
//! health.record_latency(12.0);
//! health.record_latency(18.0);
//! assert_eq!(health.status(), Status::Active);
//! assert!(health.latency().is_some());
//! ```

#![warn(missing_docs)]

pub mod ema;
pub mod load;
pub mod psi;
#[cfg(feature = "rusage")] pub mod rusage;

pub use ema::{Ema, Health, Status};
pub use load::{Load, cpu_count, parse_loadavg};
pub use psi::{Psi, PsiError, PsiLine, PsiStat, parse_pressure};
