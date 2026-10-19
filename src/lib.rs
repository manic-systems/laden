//! PSI + RSS + EMA latency telemetry for load-aware schedulers.
//!
//! `laden` gathers the signals a scheduler needs to place work on the
//! least-loaded host or agent:
//!
//! - [`psi`], kernel pressure-stall information from
//!   `/proc/pressure/{cpu,memory,io}`.
//! - [`load`], the run-queue load averages from `/proc/loadavg`.
//! - [`rss`], this process's peak resident set size.
//! - [`ema`], an exponential moving average and a latency-driven
//!   [`Health`](ema::Health) tracker.
//!
//! [`parse_pressure`](psi::parse_pressure) and
//! [`parse_loadavg`](load::parse_loadavg) are pure functions over file
//! contents, so they can be fed fixtures without touching `/proc`.
//!
//! # Example
//!
//! ```
//! use laden::ema::{
//!    Health,
//!    Status,
//! };
//!
//! let mut health = Health::default();
//! health.record_latency(12.0);
//! health.record_latency(18.0);
//! assert_eq!(health.status(), Status::Active);
//! assert!(health.latency().is_some());
//! ```

#![warn(missing_docs)]

pub mod ema;
pub mod load;
pub mod psi;
pub mod rss;
