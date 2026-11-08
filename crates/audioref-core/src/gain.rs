//! Levels in dB, and ramps that fade a value over [`RAMP_SECONDS`], so changing the level, the
//! bypass or the correction doesn't click.

/// How long a change of the level or of the bypass takes, in seconds.
pub const RAMP_SECONDS: f32 = 0.02;

/// The linear factor of a level in dB, e.g. about 0.5 for -6 dB.
#[must_use]
pub fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// The number of samples a ramp takes at a sample rate, at least 1.
#[must_use]
pub fn ramp_samples(sample_rate: f32) -> u16 {
    let samples = (sample_rate * RAMP_SECONDS)
        .round()
        .clamp(1.0, f32::from(u16::MAX));
    // NaN, e.g. of a host that reports no sample rate, clamps to NaN, which casts to 0.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 1..=u16::MAX before"
    )]
    let samples = samples as u16;
    samples.max(1)
}

/// A value that moves linearly to its target in a fixed number of samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    current: f32,
    target: f32,
    step: f32,
    remaining: u16,
    length: u16,
}

impl Ramp {
    /// A ramp that rests at `value` and takes `length` samples, at least 1, for each change.
    #[must_use]
    pub fn new(value: f32, length: u16) -> Self {
        Self {
            current: value,
            target: value,
            step: 0.0,
            remaining: 0,
            length: length.max(1),
        }
    }

    /// The value of the last sample.
    #[must_use]
    pub fn value(&self) -> f32 {
        self.current
    }

    /// Sets the number of samples, at least 1, that the next change takes.
    pub fn set_length(&mut self, length: u16) {
        self.length = length.max(1);
    }

    /// Starts moving to `target`, unless that is the target already.
    pub fn set_target(&mut self, target: f32) {
        if target.to_bits() == self.target.to_bits() {
            return;
        }
        self.target = target;
        self.remaining = self.length;
        self.step = (target - self.current) / f32::from(self.length);
    }

    /// Jumps to `from` and starts moving to `to`, e.g. for a cross-fade that starts over.
    pub fn restart(&mut self, from: f32, to: f32) {
        self.current = from;
        self.target = from;
        self.set_target(to);
    }

    /// Jumps to the target, e.g. when playback starts again.
    pub fn finish(&mut self) {
        self.current = self.target;
        self.remaining = 0;
    }

    /// Moves one sample on and returns the new value.
    pub fn next_value(&mut self) -> f32 {
        if let Some(remaining) = self.remaining.checked_sub(1) {
            self.remaining = remaining;
            self.current = if remaining == 0 {
                self.target
            } else {
                self.current + self.step
            };
        }
        self.current
    }

    /// Moves `samples` samples on.
    pub fn advance(&mut self, samples: usize) {
        for _ in 0..samples.min(usize::from(self.remaining)) {
            self.next_value();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Ramp, db_to_gain, ramp_samples};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn decibels_to_factors() {
        assert!(close(db_to_gain(0.0), 1.0));
        assert!(close(db_to_gain(20.0), 10.0));
        assert!(close(db_to_gain(-6.0), 0.501_187_2));
    }

    #[test]
    fn ramp_lengths() {
        assert_eq!(ramp_samples(48_000.0), 960);
        assert_eq!(ramp_samples(192_000.0), 3840);
        assert_eq!(ramp_samples(44_100.0), 882);
        assert_eq!(ramp_samples(0.0), 1);
        assert_eq!(ramp_samples(-1.0), 1);
        assert_eq!(ramp_samples(f32::NAN), 1);
        assert_eq!(ramp_samples(f32::INFINITY), u16::MAX);
    }

    #[test]
    fn a_ramp_moves_linearly_and_stops_at_its_target() {
        let mut ramp = Ramp::new(0.0, 4);
        ramp.set_target(1.0);
        let values: Vec<f32> = (0..6).map(|_| ramp.next_value()).collect();
        assert_eq!(values, [0.25, 0.5, 0.75, 1.0, 1.0, 1.0]);
        assert!(close(ramp.value(), 1.0));
    }

    #[test]
    fn the_same_target_again_keeps_the_ramp_going() {
        let mut ramp = Ramp::new(0.0, 4);
        ramp.set_target(1.0);
        ramp.next_value();
        ramp.set_target(1.0);
        assert!(close(ramp.next_value(), 0.5));
    }

    #[test]
    fn a_ramp_takes_at_least_one_sample() {
        let mut ramp = Ramp::new(0.0, 0);
        ramp.set_length(0);
        ramp.set_target(2.0);
        assert!(close(ramp.next_value(), 2.0));
    }

    #[test]
    fn a_ramp_restarts_from_a_value() {
        let mut ramp = Ramp::new(1.0, 2);
        ramp.restart(0.0, 1.0);
        assert!(close(ramp.value(), 0.0));
        assert!(close(ramp.next_value(), 0.5));
        assert!(close(ramp.next_value(), 1.0));
        // Restarting to where it is already rests there.
        ramp.restart(1.0, 1.0);
        assert!(close(ramp.next_value(), 1.0));
    }

    #[test]
    fn finish_and_advance() {
        let mut ramp = Ramp::new(0.0, 10);
        ramp.set_target(1.0);
        ramp.finish();
        assert!(close(ramp.value(), 1.0));
        assert!(close(ramp.next_value(), 1.0));

        let mut a = Ramp::new(0.0, 10);
        let mut b = a;
        a.set_target(1.0);
        b.set_target(1.0);
        a.advance(3);
        for _ in 0..3 {
            b.next_value();
        }
        assert_eq!(a, b);
        a.advance(100);
        assert!(close(a.value(), 1.0));
    }
}
