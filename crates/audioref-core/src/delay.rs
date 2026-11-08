//! A delay line of one channel, so the paths of the engine line up with a linear-phase
//! correction, which delays the signal.
//!
//! It allocates once, when it is made, and never in [`Delay::push`] or [`Delay::get`], so the
//! audio thread can use it.
//!
//! ```
//! use audioref_core::delay::Delay;
//!
//! let mut delay = Delay::new(4);
//! for sample in [1.0, 2.0, 3.0] {
//!     delay.push(sample);
//! }
//! assert_eq!(delay.get(0), 3.0);
//! assert_eq!(delay.get(2), 1.0);
//! assert_eq!(delay.get(3), 0.0);
//! ```

/// A delay line of one channel, of a fixed length.
#[derive(Debug, Clone, PartialEq)]
pub struct Delay {
    /// The latest samples, in a ring.
    buffer: Vec<f64>,
    /// Where the latest sample is.
    position: usize,
}

impl Delay {
    /// A delay line of silence that delays by up to `longest` samples.
    #[must_use]
    pub fn new(longest: usize) -> Self {
        Self {
            buffer: vec![0.0; longest.saturating_add(1)],
            position: 0,
        }
    }

    /// Takes in the next sample.
    pub fn push(&mut self, sample: f64) {
        self.position = (self.position + 1) % self.buffer.len();
        if let Some(slot) = self.buffer.get_mut(self.position) {
            *slot = sample;
        }
    }

    /// The sample taken in so many samples ago: 0 is the latest. Delays beyond the longest give
    /// the oldest sample kept.
    #[must_use]
    pub fn get(&self, delay: usize) -> f64 {
        let length = self.buffer.len();
        let delay = delay.min(length - 1);
        let index = (self.position + length - delay) % length;
        self.buffer.get(index).copied().unwrap_or_default()
    }

    /// Fills it with silence.
    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, reason = "samples taken over unchanged")]
mod tests {
    use super::Delay;

    #[test]
    fn delays_by_whole_samples_around_its_ring() {
        let mut delay = Delay::new(3);
        for sample in 1..=10 {
            delay.push(f64::from(sample));
            assert_eq!(delay.get(0), f64::from(sample));
            if sample > 3 {
                assert_eq!(delay.get(3), f64::from(sample - 3));
            }
        }
        // Longer than it keeps: the oldest.
        assert_eq!(delay.get(100), 7.0);
        delay.reset();
        assert_eq!(
            delay,
            Delay {
                position: delay.position,
                ..Delay::new(3)
            }
        );
        assert_eq!(delay.get(1), 0.0);
    }

    #[test]
    fn without_delay_gives_the_latest() {
        let mut delay = Delay::new(0);
        delay.push(0.5);
        assert_eq!(delay.get(0), 0.5);
        assert_eq!(delay.get(1), 0.5);
    }
}
