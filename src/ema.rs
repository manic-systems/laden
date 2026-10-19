//! Exponential moving average and the latency-driven health tracker.

/// The smoothing factor used by [`Ema::default`] and [`Health::new`].
pub const DEFAULT_ALPHA: f64 = 0.3;

const DEGRADED_FAILS: u32 = 3;
const DOWN_FAILS: u32 = 10;

/// An exponential moving average.
///
/// Each [`record`](Ema::record) blends a new sample into the running value with
/// weight `alpha`; the first sample seeds the average unchanged. A larger
/// `alpha` tracks recent samples more aggressively, a smaller one smooths more
/// heavily.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ema {
  alpha: f64,
  value: Option<f64>,
}

impl Ema {
  /// Create an empty average with the given smoothing factor, in `0.0..=1.0`.
  #[must_use]
  pub fn new(alpha: f64) -> Self {
    Self { alpha, value: None }
  }

  /// Blend `sample` into the running average, seeding it on the first call.
  pub fn record(&mut self, sample: f64) {
    self.value = Some(match self.value {
      Some(prev) => self.alpha * sample + (1.0 - self.alpha) * prev,
      None => sample,
    });
  }

  /// The current average, or `None` if no sample has been recorded.
  #[must_use]
  pub fn get(&self) -> Option<f64> {
    self.value
  }
}

impl Default for Ema {
  fn default() -> Self {
    Self::new(DEFAULT_ALPHA)
  }
}

/// The coarse health classification derived from consecutive failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
  /// Below the degraded threshold — healthy.
  Active,
  /// At or above the degraded threshold but below the down threshold.
  Degraded,
  /// At or above the down threshold.
  Down,
}

/// A latency-smoothing health tracker.
///
/// Successful observations feed [`record_latency`](Health::record_latency),
/// which both updates the smoothed latency and clears the consecutive-failure
/// run. Failures bump that run via [`record_failure`](Health::record_failure),
/// and [`status`](Health::status) maps the run length onto a [`Status`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Health {
  latency:           Ema,
  consecutive_fails: u32,
  degraded_at:       u32,
  down_at:           u32,
}

impl Health {
  /// Create a tracker with the default smoothing factor and failure
  /// thresholds.
  #[must_use]
  pub fn new() -> Self {
    Self::with_alpha(DEFAULT_ALPHA)
  }

  /// Create a tracker with a custom smoothing factor and the default failure
  /// thresholds.
  #[must_use]
  pub fn with_alpha(alpha: f64) -> Self {
    Self {
      latency:           Ema::new(alpha),
      consecutive_fails: 0,
      degraded_at:       DEGRADED_FAILS,
      down_at:           DOWN_FAILS,
    }
  }

  /// Create a tracker with custom consecutive-failure thresholds.
  ///
  /// `degraded` is the run length at which [`status`](Health::status) reports
  /// [`Status::Degraded`], and `down` the length at which it reports
  /// [`Status::Down`]. The smoothing factor is left at [`DEFAULT_ALPHA`]; the
  /// thresholds are used as given even if `down <= degraded`, in which case the
  /// tracker can step straight from [`Status::Active`] to [`Status::Down`].
  #[must_use]
  pub fn with_thresholds(degraded: u32, down: u32) -> Self {
    Self {
      latency:           Ema::new(DEFAULT_ALPHA),
      consecutive_fails: 0,
      degraded_at:       degraded,
      down_at:           down,
    }
  }

  /// Record a successful latency sample, resetting the failure run.
  pub fn record_latency(&mut self, sample: f64) {
    self.latency.record(sample);
    self.consecutive_fails = 0;
  }

  /// Record a failed observation, extending the failure run.
  pub fn record_failure(&mut self) {
    self.consecutive_fails = self.consecutive_fails.saturating_add(1);
  }

  /// The smoothed latency, or `None` before the first success.
  #[must_use]
  pub fn latency(&self) -> Option<f64> {
    self.latency.get()
  }

  /// The current consecutive-failure count.
  #[must_use]
  pub fn consecutive_fails(&self) -> u32 {
    self.consecutive_fails
  }

  /// Classify health from the current failure run.
  #[must_use]
  pub fn status(&self) -> Status {
    if self.consecutive_fails >= self.down_at {
      Status::Down
    } else if self.consecutive_fails >= self.degraded_at {
      Status::Degraded
    } else {
      Status::Active
    }
  }
}

impl Default for Health {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ema_converges() {
    let mut ema = Ema::new(0.3);
    ema.record(100.0);
    ema.record(50.0);
    let value = ema.get().expect("value after two samples");
    assert!((value - 85.0).abs() < 1e-9, "expected ~85.0, got {value}");
  }

  #[test]
  fn ema_first_sample_is_seed() {
    let mut ema = Ema::default();
    assert_eq!(ema.get(), None);
    ema.record(7.0);
    assert_eq!(ema.get(), Some(7.0));
  }

  #[test]
  fn status_thresholds() {
    let mut health = Health::new();
    assert_eq!(health.status(), Status::Active);

    for _ in 0..3 {
      health.record_failure();
    }
    assert_eq!(health.status(), Status::Degraded);

    for _ in 0..7 {
      health.record_failure();
    }
    assert_eq!(health.status(), Status::Down);
  }

  #[test]
  fn latency_resets_failures() {
    let mut health = Health::new();
    for _ in 0..5 {
      health.record_failure();
    }
    assert_eq!(health.status(), Status::Degraded);
    health.record_latency(10.0);
    assert_eq!(health.status(), Status::Active);
    assert_eq!(health.consecutive_fails(), 0);
  }

  #[test]
  fn custom_thresholds_apply() {
    let mut health = Health::with_thresholds(1, 2);
    assert_eq!(health.status(), Status::Active);
    health.record_failure();
    assert_eq!(health.status(), Status::Degraded);
    health.record_failure();
    assert_eq!(health.status(), Status::Down);
  }
}
