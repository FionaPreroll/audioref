//! The processing of the plugin: the correction, the output level and the bypass, each faded
//! over [`RAMP_SECONDS`](crate::gain::RAMP_SECONDS), so changing them doesn't click.
//!
//! A new correction is designed in the background as a [`Filter`], the biquads of a preset or the
//! FIR of a designed correction, and handed to [`Engine::set_correction`], which cross-fades from
//! the one before. The engine never allocates or frees: it hands back the filter it no longer
//! needs, to be freed outside the audio thread.
//!
//! ```
//! use audioref_core::cascade::Cascade;
//! use audioref_core::engine::Engine;
//! use audioref_core::eq::ParametricEq;
//! use audioref_core::filter::Filter;
//!
//! let mut engine = Engine::new(48_000.0);
//! engine.set_output_db(-6.0);
//! let eq = ParametricEq::new(-3.0, Vec::new())?;
//! let filter = Filter::from(Cascade::new(&eq, 48_000.0));
//! assert!(engine.set_correction(Some(Box::new(filter))).is_none());
//! engine.reset();
//! let mut left = [1.0_f32; 4];
//! let mut right = [0.5_f32; 4];
//! engine.process(&mut [&mut left, &mut right]);
//! assert!((left[0] - 0.355).abs() < 0.001); // -9 dB
//! # Ok::<(), audioref_core::eq::EqError>(())
//! ```

use crate::filter::Filter;
use crate::gain::{Ramp, db_to_gain, ramp_samples};

/// The processing of the plugin: the correction, the output level and the bypass.
#[derive(Debug, Clone, PartialEq)]
pub struct Engine {
    gain: Ramp,
    /// 1 while processing, 0 while bypassed.
    wet: Ramp,
    /// From 0, the previous correction, to 1, the current one.
    fade: Ramp,
    /// The correction, none for a flat response.
    current: Option<Box<Filter>>,
    /// The correction before, while the fade runs, and kept until the next one comes, so the
    /// audio thread never frees it.
    previous: Option<Box<Filter>>,
}

impl Engine {
    /// An engine at 0 dB, without a correction, not bypassed, for a sample rate in Hz.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let length = ramp_samples(sample_rate);
        Self {
            gain: Ramp::new(1.0, length),
            wet: Ramp::new(1.0, length),
            fade: Ramp::new(1.0, length),
            current: None,
            previous: None,
        }
    }

    /// Adapts the length of the fades to a new sample rate in Hz. The correction must be designed
    /// again for it.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let length = ramp_samples(sample_rate);
        self.gain.set_length(length);
        self.wet.set_length(length);
        self.fade.set_length(length);
    }

    /// Fades to an output level in dB.
    pub fn set_output_db(&mut self, db: f32) {
        self.gain.set_target(db_to_gain(db));
    }

    /// Fades to the unchanged signal, or back to the processed one.
    pub fn set_bypass(&mut self, bypass: bool) {
        self.wet.set_target(if bypass { 0.0 } else { 1.0 });
    }

    /// Cross-fades to a correction, or to none, and hands back the one it kept from before,
    /// to be freed outside the audio thread. A fade that still runs starts over from the
    /// correction that was current.
    pub fn set_correction(&mut self, correction: Option<Box<Filter>>) -> Option<Box<Filter>> {
        let current = std::mem::replace(&mut self.current, correction);
        let stale = std::mem::replace(&mut self.previous, current);
        self.fade.restart(0.0, 1.0);
        stale
    }

    /// The current correction, if there is one.
    #[must_use]
    pub fn correction(&self) -> Option<&Filter> {
        self.current.as_deref()
    }

    /// Ends all fades at once and empties the filters, e.g. when playback starts again.
    pub fn reset(&mut self) {
        self.gain.finish();
        self.wet.finish();
        self.fade.finish();
        for filter in [&mut self.current, &mut self.previous]
            .into_iter()
            .flatten()
        {
            filter.reset();
        }
    }

    /// Processes one block in place: one slice per channel, all of the same length.
    pub fn process(&mut self, channels: &mut [&mut [f32]]) {
        let samples = channels.first().map_or(0, |channel| channel.len());
        for (index, channel) in channels.iter_mut().enumerate() {
            // Each channel runs through the same fades, which then move on once for the block.
            let mut gain = self.gain;
            let mut wet = self.wet;
            let mut fade = self.fade;
            for sample in channel.iter_mut() {
                let input = f64::from(*sample);
                let mix = fade.next_value();
                let new = self
                    .current
                    .as_deref_mut()
                    .map_or(input, |filter| filter.process(index, input));
                let corrected = if mix < 1.0 {
                    let old = self
                        .previous
                        .as_deref_mut()
                        .map_or(input, |filter| filter.process(index, input));
                    old + f64::from(mix) * (new - old)
                } else {
                    new
                };
                let wet = f64::from(wet.next_value());
                let processed = f64::from(gain.next_value()) * corrected;
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "the host takes 32-bit samples; the filters run with 64 bits"
                )]
                let output = (input + wet * (processed - input)) as f32;
                *sample = output;
            }
        }
        self.gain.advance(samples);
        self.wet.advance(samples);
        self.fade.advance(samples);
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;
    use crate::cascade::Cascade;
    use crate::eq::{Band, ParametricEq};
    use crate::filter::Filter;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    /// A cascade at 200 Hz, so a fade takes 4 samples, of a flat EQ with an overall gain.
    fn flat(gain_db: f64) -> Filter {
        let eq = ParametricEq::new(gain_db, Vec::new()).expect("accepted");
        Filter::from(Cascade::new(&eq, 200.0))
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

    #[test]
    fn a_correction_fades_in_and_the_one_before_comes_back_later() {
        let mut engine = Engine::new(200.0);
        // -20 dB: a factor of 0.1.
        assert!(engine.set_correction(Some(Box::new(flat(-20.0)))).is_none());
        let mut samples = [1.0_f32; 5];
        engine.process(&mut [&mut samples]);
        assert!(close(samples[0], 0.775));
        assert!(close(samples[3], 0.1));
        assert!(close(samples[4], 0.1));
        assert!(engine.correction().is_some_and(
            |filter| matches!(filter, Filter::Biquads(cascade) if cascade.headroom_db() == 0.0)
        ));

        // The next one fades from this one, which the engine keeps until the one after.
        assert!(engine.set_correction(Some(Box::new(flat(0.0)))).is_none());
        let mut samples = [1.0_f32; 4];
        engine.process(&mut [&mut samples]);
        assert!(close(samples[0], 0.325));
        assert!(close(samples[3], 1.0));
        let stale = engine.set_correction(None);
        assert!(stale.is_some_and(|filter| matches!(*filter, Filter::Biquads(cascade) if (cascade.response_db(100.0) + 20.0).abs() < 1e-9)));
        assert!(engine.correction().is_none());
    }

    #[test]
    fn a_reset_ends_the_fade_of_a_correction() {
        let mut engine = Engine::new(200.0);
        let eq = ParametricEq::new(
            0.0,
            vec![Band::Peak {
                frequency: 50.0,
                gain_db: -6.0,
                q: 1.0,
            }],
        )
        .expect("accepted");
        engine.set_correction(Some(Box::new(Filter::from(Cascade::new(&eq, 200.0)))));
        let mut warm = [1.0_f32; 3];
        engine.process(&mut [&mut warm]);
        engine.reset();
        let mut first = [1.0_f32, 0.0, 0.0];
        engine.process(&mut [&mut first]);
        // Reset again, the filter starts from the same, empty state.
        engine.reset();
        let mut second = [1.0_f32, 0.0, 0.0];
        engine.process(&mut [&mut second]);
        assert!(first.iter().zip(&second).all(|(a, b)| close(*a, *b)));
    }
}
