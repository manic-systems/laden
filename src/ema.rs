//! Exponential moving average and the latency-driven health tracker.

/// A smoothing factor for [`Ema`], in `0.0..=1.0`.
///
/// A larger factor tracks recent samples more aggressively, a smaller one
/// smooths more heavily.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Alpha(f64);

impl Alpha {
   /// The smoothing factor used by [`Ema::default`] and [`Health::default`].
   pub const DEFAULT: Self = Self(0.3);
}

impl Default for Alpha {
   #[inline]
   fn default() -> Self {
      Self::DEFAULT
   }
}

impl TryFrom<f64> for Alpha {
   type Error = AlphaError;

   #[inline]
   fn try_from(value: f64) -> Result<Self, AlphaError> {
      if (0.0..=1.0).contains(&value) {
         Ok(Self(value))
      } else {
         Err(AlphaError(value))
      }
   }
}

/// A smoothing factor outside `0.0..=1.0`, or NaN.
#[derive(Debug, Clone, Copy, PartialEq, flub::Error)]
#[error("smoothing factor {0} is outside 0.0..=1.0")]
pub struct AlphaError(pub f64);

/// An exponential moving average.
///
/// Each [`record`](Ema::record) blends a new sample into the running value with
/// weight `alpha`, and the first sample seeds the average unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ema {
   /// Weight given to each new sample.
   alpha: Alpha,
   /// The running average, or `None` before the first sample.
   value: Option<f64>,
}

impl Ema {
   /// Create an empty average with the given smoothing factor.
   #[inline]
   #[must_use]
   pub const fn new(alpha: Alpha) -> Self {
      Self { alpha, value: None }
   }

   /// Blend `sample` into the running average, seeding it on the first call.
   #[inline]
   #[expect(clippy::float_arithmetic, reason = "an average is floating-point math")]
   pub fn record(&mut self, sample: f64) {
      self.value = Some(match self.value {
         Some(prev) => (1.0_f64 - self.alpha.0).mul_add(prev, self.alpha.0 * sample),
         None => sample,
      });
   }

   /// The current average, or `None` if no sample has been recorded.
   #[inline]
   #[must_use]
   pub const fn get(&self) -> Option<f64> {
      self.value
   }
}

impl Default for Ema {
   #[inline]
   fn default() -> Self {
      Self::new(Alpha::DEFAULT)
   }
}

/// Consecutive-failure run lengths at which [`Health`] changes [`Status`].
///
/// They are used as given, so `down <= degraded` steps straight from
/// [`Status::Active`] to [`Status::Down`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
   /// Run length at which [`Status::Degraded`] begins.
   pub degraded: u32,
   /// Run length at which [`Status::Down`] begins.
   pub down:     u32,
}

impl Default for Thresholds {
   #[inline]
   fn default() -> Self {
      Self {
         degraded: 3,
         down:     10,
      }
   }
}

/// The coarse health classification derived from consecutive failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
   /// Below the degraded threshold.
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
   /// Smoothed latency of successful observations.
   latency:           Ema,
   /// Failures since the last success.
   consecutive_fails: u32,
   /// Where the failure run changes [`Status`].
   thresholds:        Thresholds,
}

impl Health {
   /// Create a tracker with the given smoothing factor and failure thresholds.
   #[inline]
   #[must_use]
   pub const fn new(alpha: Alpha, thresholds: Thresholds) -> Self {
      Self {
         latency: Ema::new(alpha),
         consecutive_fails: 0,
         thresholds,
      }
   }

   /// Record a successful latency sample, resetting the failure run.
   #[inline]
   pub fn record_latency(&mut self, sample: f64) {
      self.latency.record(sample);
      self.consecutive_fails = 0;
   }

   /// Record a failed observation, extending the failure run.
   #[inline]
   pub const fn record_failure(&mut self) {
      self.consecutive_fails = self.consecutive_fails.saturating_add(1);
   }

   /// The smoothed latency, or `None` before the first success.
   #[inline]
   #[must_use]
   pub const fn latency(&self) -> Option<f64> {
      self.latency.get()
   }

   /// The current consecutive-failure count.
   #[inline]
   #[must_use]
   pub const fn consecutive_fails(&self) -> u32 {
      self.consecutive_fails
   }

   /// Classify health from the current failure run.
   #[inline]
   #[must_use]
   pub const fn status(&self) -> Status {
      if self.consecutive_fails >= self.thresholds.down {
         Status::Down
      } else if self.consecutive_fails >= self.thresholds.degraded {
         Status::Degraded
      } else {
         Status::Active
      }
   }
}

impl Default for Health {
   #[inline]
   fn default() -> Self {
      Self::new(Alpha::DEFAULT, Thresholds::default())
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn ema_converges() {
      let mut ema = Ema::new(Alpha::try_from(0.3_f64).expect("0.3 is in range"));
      ema.record(100.0);
      ema.record(50.0);
      let value = ema.get().expect("value after two samples");
      assert!(
         (value - 85.0).abs() < 1.0e-9_f64,
         "expected ~85.0, got {value}"
      );
   }

   #[test]
   fn ema_first_sample_is_seed() {
      let mut ema = Ema::default();
      assert_eq!(ema.get(), None);
      ema.record(7.0);
      assert_eq!(ema.get(), Some(7.0_f64));
   }

   #[test]
   fn status_thresholds() {
      let mut health = Health::default();
      assert_eq!(health.status(), Status::Active);

      for _ in 0_u8..3 {
         health.record_failure();
      }
      assert_eq!(health.status(), Status::Degraded);

      for _ in 0_u8..7 {
         health.record_failure();
      }
      assert_eq!(health.status(), Status::Down);
   }

   #[test]
   fn latency_resets_failures() {
      let mut health = Health::default();
      for _ in 0_u8..5 {
         health.record_failure();
      }
      assert_eq!(health.status(), Status::Degraded);
      health.record_latency(10.0);
      assert_eq!(health.status(), Status::Active);
      assert_eq!(health.consecutive_fails(), 0);
   }

   #[test]
   fn custom_thresholds_apply() {
      let mut health = Health::new(Alpha::DEFAULT, Thresholds {
         degraded: 1,
         down:     2,
      });
      assert_eq!(health.status(), Status::Active);
      health.record_failure();
      assert_eq!(health.status(), Status::Degraded);
      health.record_failure();
      assert_eq!(health.status(), Status::Down);
   }
}
