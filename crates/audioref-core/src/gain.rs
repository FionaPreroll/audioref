//! The output level and the bypass, both faded over [`RAMP_SECONDS`], so changing them doesn't
//! click.
//!
//! ```
//! use audioref_core::gain::Engine;
//!
//! let mut engine = Engine::new(48_000.0);
//! engine.set_output_db(-6.0);
//! engine.reset();
//! let mut left = [1.0_f32; 4];
//! let mut right = [0.5_f32; 4];
//! engine.process(&mut [&mut left, &mut right]);
//! assert!((left[0] - 0.501).abs() < 0.001);
//! ```

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

/// The processing of the plugin so far: the output level and the bypass.
#[derive(Debug, Clone, PartialEq)]
pub struct Engine {
    gain: Ramp,
    /// 1 while processing, 0 while bypassed.
    wet: Ramp,
}

impl Engine {
    /// An engine at 0 dB, not bypassed, for a sample rate in Hz.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let length = ramp_samples(sample_rate);
        Self {
            gain: Ramp::new(1.0, length),
            wet: Ramp::new(1.0, length),
        }
    }

    /// Adapts the length of the fades to a new sample rate in Hz.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let length = ramp_samples(sample_rate);
        self.gain.set_length(length);
        self.wet.set_length(length);
    }

    /// Fades to an output level in dB.
    pub fn set_output_db(&mut self, db: f32) {
        self.gain.set_target(db_to_gain(db));
    }

    /// Fades to the unchanged signal, or back to the processed one.
    pub fn set_bypass(&mut self, bypass: bool) {
        self.wet.set_target(if bypass { 0.0 } else { 1.0 });
    }

    /// Ends all fades at once, e.g. when playback starts again.
    pub fn reset(&mut self) {
        self.gain.finish();
        self.wet.finish();
    }

    /// Processes one block in place: one slice per channel, all of the same length.
    pub fn process(&mut self, channels: &mut [&mut [f32]]) {
        let samples = channels.first().map_or(0, |channel| channel.len());
        for channel in channels.iter_mut() {
            // Each channel runs through the same fades, which then move on once for the block.
            let mut gain = self.gain;
            let mut wet = self.wet;
            for sample in channel.iter_mut() {
                let wet = wet.next_value();
                *sample *= 1.0 + wet * (gain.next_value() - 1.0);
            }
        }
        self.gain.advance(samples);
        self.wet.advance(samples);
    }
}

#[cfg(test)]
mod tests {
    use super::{Engine, Ramp, db_to_gain, ramp_samples};

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

    #[test]
    #[allow(clippy::float_cmp, reason = "unchanged means the very same samples")]
    fn unity_passes_the_signal_through() {
        let mut engine = Engine::new(48_000.0);
        let mut left = [0.1, -0.2, 0.3];
        let mut right = [1.0, 0.0, -1.0];
        engine.process(&mut [&mut left, &mut right]);
        assert_eq!(left, [0.1, -0.2, 0.3]);
        assert_eq!(right, [1.0, 0.0, -1.0]);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "both channels must get the very same factors"
    )]
    fn the_level_fades_the_same_in_every_channel() {
        let mut engine = Engine::new(200.0); // 4 samples per fade
        engine.set_output_db(20.0);
        let mut left = [1.0_f32; 6];
        let mut right = [1.0_f32; 6];
        engine.process(&mut [&mut left, &mut right]);
        assert_eq!(left, right);
        assert!(close(left[0], 3.25));
        assert!(close(left[3], 10.0));
        assert!(close(left[5], 10.0));

        // The next block goes on where the fade ended.
        let mut mono = [1.0_f32; 2];
        engine.process(&mut [&mut mono]);
        assert!(close(mono[0], 10.0));
    }

    #[test]
    fn bypass_fades_to_the_unchanged_signal() {
        let mut engine = Engine::new(200.0);
        engine.set_output_db(20.0);
        engine.reset();
        engine.set_bypass(true);
        let mut samples = [1.0_f32; 5];
        engine.process(&mut [&mut samples]);
        assert!(close(samples[0], 7.75));
        assert!(close(samples[3], 1.0));
        assert!(close(samples[4], 1.0));
    }

    #[test]
    fn a_new_sample_rate_changes_the_length_of_the_fades() {
        let mut engine = Engine::new(48_000.0);
        engine.set_sample_rate(100.0); // 2 samples per fade
        engine.set_output_db(20.0);
        let mut samples = [1.0_f32; 3];
        engine.process(&mut [&mut samples]);
        assert!(close(samples[0], 5.5));
        assert!(close(samples[1], 10.0));
    }

    #[test]
    fn no_channels_and_empty_blocks() {
        let mut engine = Engine::new(48_000.0);
        engine.set_output_db(-6.0);
        engine.process(&mut []);
        let mut empty: [f32; 0] = [];
        engine.process(&mut [&mut empty]);
        let before = engine.clone();
        engine.process(&mut []);
        assert_eq!(engine, before);
    }
}
