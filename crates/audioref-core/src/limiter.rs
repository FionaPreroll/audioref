//! A limiter at the end of the chain, so a boost of the output level or of a correction never
//! clips the signal: both channels together are kept at or below [`CEILING_DB`].
//!
//! It works without lookahead, so it adds no latency: when a sample would go above the ceiling,
//! the gain drops to just what keeps it there, at once, and comes back over [`RELEASE_SECONDS`].
//! Off, it only lets the gain come back, so turning it off doesn't click. It notes the most it
//! lowered the signal, for the window's display, see [`Limiter::take_reduction_db`].
//!
//! ```
//! use audioref_core::limiter::Limiter;
//!
//! let mut limiter = Limiter::new(48_000.0);
//! let (left, right) = limiter.process(2.0, -0.5);
//! assert!((left - 0.966).abs() < 1e-3); // -0.3 dBFS
//! assert!((right + 0.241).abs() < 1e-3);
//! assert!((limiter.take_reduction_db() - 6.32).abs() < 0.01);
//! // A second of quiet later, the gain is back.
//! for _ in 0..48_000 {
//!     limiter.process(0.0, 0.0);
//! }
//! limiter.take_reduction_db();
//! assert!(limiter.take_reduction_db() < 1e-3);
//! ```

/// The highest level the limiter lets through, in dBFS.
pub const CEILING_DB: f64 = -0.3;

/// How long the gain takes to come back most of the way (to within 1/e), in seconds.
pub const RELEASE_SECONDS: f64 = 0.1;

/// The limiter, with its gain.
#[derive(Debug, Clone, PartialEq)]
pub struct Limiter {
    /// Whether it limits.
    enabled: bool,
    /// The ceiling as a factor.
    ceiling: f64,
    /// The share by which the gain comes back each sample.
    release: f64,
    /// The current gain, from 0 to 1.
    gain: f64,
    /// The lowest gain since the reduction was last taken.
    lowest: f64,
}

impl Limiter {
    /// A limiter that limits, at a sample rate in Hz.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self {
            enabled: true,
            ceiling: 10.0_f64.powf(CEILING_DB / 20.0),
            release: release(sample_rate),
            gain: 1.0,
            lowest: 1.0,
        }
    }

    /// Adapts the release to a new sample rate in Hz.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.release = release(sample_rate);
    }

    /// Turns the limiter on or off.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Lets the gain come back at once, e.g. when playback starts again.
    pub fn reset(&mut self) {
        self.gain = 1.0;
    }

    /// Processes one sample of each channel.
    pub fn process(&mut self, left: f64, right: f64) -> (f64, f64) {
        let peak = left.abs().max(right.abs());
        let target = if self.enabled && peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        if target < self.gain {
            self.gain = target;
        } else {
            self.gain += self.release * (target - self.gain);
        }
        self.lowest = self.lowest.min(self.gain);
        (left * self.gain, right * self.gain)
    }

    /// The most the limiter lowered the signal since this was last asked, in dB, 0 or more.
    pub fn take_reduction_db(&mut self) -> f64 {
        let lowest = std::mem::replace(&mut self.lowest, self.gain);
        if lowest < 1.0 {
            -20.0 * lowest.log10()
        } else {
            0.0
        }
    }
}

/// The share by which the gain comes back each sample at a sample rate: all of it for a sample
/// rate that doesn't fit.
fn release(sample_rate: f32) -> f64 {
    let release = 1.0 - (-1.0 / (RELEASE_SECONDS * f64::from(sample_rate))).exp();
    if release.is_finite() && release > 0.0 {
        release.min(1.0)
    } else {
        1.0
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "a signal below the ceiling passes exactly"
)]
mod tests {
    use super::*;

    #[test]
    fn below_the_ceiling_the_signal_passes() {
        let mut limiter = Limiter::new(48_000.0);
        for x in [0.0, 0.5, -0.9, 0.96] {
            assert_eq!(limiter.process(x, -x), (x, -x));
        }
        assert_eq!(limiter.take_reduction_db(), 0.0);
    }

    #[test]
    fn nothing_goes_above_the_ceiling_and_the_gain_comes_back() {
        let ceiling = 10.0_f64.powf(CEILING_DB / 20.0);
        let mut limiter = Limiter::new(48_000.0);
        for n in 0..1000 {
            let x = 4.0 * (f64::from(n) * 0.05).sin();
            let (left, right) = limiter.process(x, 0.5 * x);
            assert!(left.abs() <= ceiling + 1e-12 && right.abs() <= ceiling + 1e-12);
        }
        let reduction = limiter.take_reduction_db();
        assert!(
            (reduction - 20.0 * (4.0 / ceiling).log10()).abs() < 0.01,
            "{reduction}"
        );
        // After a second of quiet the gain is back, and the next reduction is small.
        for _ in 0..48_000 {
            limiter.process(0.0, 0.0);
        }
        assert!(limiter.take_reduction_db() > 0.0);
        assert!(limiter.take_reduction_db() < 1e-3);
        let (left, _) = limiter.process(0.5, 0.0);
        assert!((left - 0.5).abs() < 1e-3);
    }

    #[test]
    fn off_it_only_lets_the_gain_come_back() {
        let mut limiter = Limiter::new(100.0);
        limiter.process(2.0, 0.0);
        limiter.set_enabled(false);
        let (first, _) = limiter.process(2.0, 0.0);
        let (second, _) = limiter.process(2.0, 0.0);
        assert!(first < second && second < 2.0);
        limiter.reset();
        assert_eq!(limiter.process(2.0, 0.0), (2.0, 0.0));
        limiter.set_enabled(true);
        limiter.set_sample_rate(48_000.0);
        assert!(limiter.process(2.0, 0.0).0 < 1.0);
    }

    #[test]
    fn sample_rates_the_release_doesnt_fit() {
        for sample_rate in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(release(sample_rate), 1.0, "{sample_rate}");
        }
        let at_48k = release(48_000.0);
        assert!(at_48k > 2e-4 && at_48k < 2.2e-4);
    }
}
