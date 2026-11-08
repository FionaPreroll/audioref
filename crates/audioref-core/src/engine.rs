//! The processing of the plugin: the stereo stage ([`crate::stereo`]), the correction, the output
//! level, the limiter ([`crate::limiter`]) and the bypass, each faded over
//! [`RAMP_SECONDS`](crate::gain::RAMP_SECONDS), so changing them doesn't click.
//!
//! A new correction is designed in the background as a [`Filter`], the biquads of a preset or the
//! FIR of a designed correction, and handed to [`Engine::set_correction`], which cross-fades from
//! the one before. The engine never allocates or frees: it hands back the filter it no longer
//! needs, to be freed outside the audio thread.
//!
//! With a latency ([`Engine::set_latency`]), e.g. for linear-phase corrections, the engine delays
//! every path by as much: the unchanged signal of the bypass, the signal without a correction, and
//! each correction by the latency less its own. So the bypass, the fades and corrections of either
//! phase stay in line, and the host can make up for one latency that doesn't change with them.
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

use crate::delay::Delay;
use crate::filter::Filter;
use crate::fir::MAX_LATENCY;
use crate::gain::{Ramp, db_to_gain, ramp_samples};
use crate::limiter::Limiter;
use crate::stereo::{Channels, Crossfeed, Stereo};

/// The processing of the plugin: the stereo stage, the correction, the output level, the limiter
/// and the bypass.
#[derive(Debug, Clone, PartialEq)]
pub struct Engine {
    /// The routing of the channels and the crossfeed.
    stereo: Stereo,
    /// The limiter after the output level.
    limiter: Limiter,
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
    /// The highest absolute sample of the output since it was last taken.
    peak: f32,
    /// By how much every path is delayed, in samples.
    latency: usize,
    /// The input, left and right, for the bypass.
    dry: [Delay; 2],
    /// The output of the stereo stage, left and right, for the corrections.
    routed: [Delay; 2],
}

impl Engine {
    /// An engine at 0 dB, in stereo without crossfeed, without a correction or the limiter, not
    /// bypassed, for a sample rate in Hz.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let length = ramp_samples(sample_rate);
        let mut limiter = Limiter::new(sample_rate);
        limiter.set_enabled(false);
        Self {
            stereo: Stereo::new(sample_rate),
            limiter,
            gain: Ramp::new(1.0, length),
            wet: Ramp::new(1.0, length),
            fade: Ramp::new(1.0, length),
            current: None,
            previous: None,
            peak: 0.0,
            latency: 0,
            dry: [Delay::new(MAX_LATENCY), Delay::new(MAX_LATENCY)],
            routed: [Delay::new(MAX_LATENCY), Delay::new(MAX_LATENCY)],
        }
    }

    /// Delays every path by a latency in samples, at most [`MAX_LATENCY`], e.g.
    /// [`Phase::latency`](crate::filter::Phase::latency) of the corrections to come. The host
    /// must be told, so it makes up for it.
    pub fn set_latency(&mut self, latency: usize) {
        self.latency = latency.min(MAX_LATENCY);
    }

    /// By how much every path is delayed, in samples.
    #[must_use]
    pub fn latency(&self) -> usize {
        self.latency
    }

    /// Adapts the length of the fades to a new sample rate in Hz. The correction must be designed
    /// again for it.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let length = ramp_samples(sample_rate);
        self.stereo.set_sample_rate(sample_rate);
        self.limiter.set_sample_rate(sample_rate);
        self.gain.set_length(length);
        self.wet.set_length(length);
        self.fade.set_length(length);
    }

    /// Fades to an output level in dB.
    pub fn set_output_db(&mut self, db: f32) {
        self.gain.set_target(db_to_gain(db));
    }

    /// Fades to a routing of the channels.
    pub fn set_channels(&mut self, channels: Channels) {
        self.stereo.set_channels(channels);
    }

    /// Fades to a strength of the crossfeed.
    pub fn set_crossfeed(&mut self, crossfeed: Crossfeed) {
        self.stereo.set_crossfeed(crossfeed);
    }

    /// Turns the limiter on or off.
    pub fn set_limiter(&mut self, enabled: bool) {
        self.limiter.set_enabled(enabled);
    }

    /// The most the limiter lowered the signal since this was last asked, in dB, see
    /// [`Limiter::take_reduction_db`].
    pub fn take_reduction_db(&mut self) -> f64 {
        self.limiter.take_reduction_db()
    }

    /// The highest absolute sample of the output since this was last asked, as a factor of full
    /// scale, for the window's display of the level.
    pub fn take_peak(&mut self) -> f32 {
        std::mem::take(&mut self.peak)
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
        self.stereo.reset();
        self.limiter.reset();
        self.gain.finish();
        self.wet.finish();
        self.fade.finish();
        for filter in [&mut self.current, &mut self.previous]
            .into_iter()
            .flatten()
        {
            filter.reset();
        }
        for delay in self.dry.iter_mut().chain(self.routed.iter_mut()) {
            delay.reset();
        }
    }

    /// Processes one block in place: one slice per channel, all of the same length. The first two
    /// channels are left and right; one alone is mono, and channels after the first two pass
    /// unchanged.
    pub fn process(&mut self, channels: &mut [&mut [f32]]) {
        match channels {
            [] => {}
            [mono] => {
                for sample in mono.iter_mut() {
                    (*sample, _) = self.frame(*sample, *sample);
                }
            }
            [left, right, ..] => {
                for (left, right) in left.iter_mut().zip(right.iter_mut()) {
                    (*left, *right) = self.frame(*left, *right);
                }
            }
        }
    }

    /// Processes one sample of each channel: the stereo stage, the correction, the level, the
    /// limiter and the bypass.
    fn frame(&mut self, left: f32, right: f32) -> (f32, f32) {
        let (left, right) = (f64::from(left), f64::from(right));
        let (routed_left, routed_right) = self.stereo.process(left, right);
        let [dry_left, dry_right] = &mut self.dry;
        let [delayed_left, delayed_right] = &mut self.routed;
        for (delay, sample) in [
            (dry_left, left),
            (dry_right, right),
            (delayed_left, routed_left),
            (delayed_right, routed_right),
        ] {
            delay.push(sample);
        }
        let [dry_left, dry_right] = &self.dry;
        let (left, right) = (dry_left.get(self.latency), dry_right.get(self.latency));
        let mix = f64::from(self.fade.next_value());
        let gain = f64::from(self.gain.next_value());
        let wet = f64::from(self.wet.next_value());
        let [routed_left, routed_right] = &self.routed;
        let mut correct = |channel, routed| {
            let filters = (self.current.as_deref_mut(), self.previous.as_deref_mut());
            correct(filters, channel, routed, self.latency, mix)
        };
        let corrected_left = correct(0, routed_left);
        let corrected_right = correct(1, routed_right);
        let (limited_left, limited_right) = self
            .limiter
            .process(gain * corrected_left, gain * corrected_right);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the host takes 32-bit samples; the filters run with 64 bits"
        )]
        let output = (
            (left + wet * (limited_left - left)) as f32,
            (right + wet * (limited_right - right)) as f32,
        );
        self.peak = self.peak.max(output.0.abs()).max(output.1.abs());
        output
    }
}

/// The latest sample of a channel out of the stereo stage, from its delay line, through the
/// current correction and the one before, each delayed so it comes out at the latency, and
/// cross-faded from the one before while `mix` is below 1.
fn correct(
    (current, previous): (Option<&mut Filter>, Option<&mut Filter>),
    channel: usize,
    routed: &Delay,
    latency: usize,
    mix: f64,
) -> f64 {
    let through = |filter: Option<&mut Filter>| match filter {
        None => routed.get(latency),
        Some(filter) => {
            let input = routed.get(latency.saturating_sub(filter.latency()));
            filter.process(channel, input)
        }
    };
    let new = through(current);
    if mix < 1.0 {
        let old = through(previous);
        old + mix * (new - old)
    } else {
        new
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;
    use crate::cascade::Cascade;
    use crate::eq::{Band, ParametricEq};
    use crate::filter::{Filter, Phase};
    use crate::fir::MAX_LATENCY;
    use crate::stereo::{Channels, Crossfeed};

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

    #[test]
    #[allow(clippy::float_cmp, reason = "routed and passed samples are exact")]
    fn the_stereo_stage_comes_first_and_further_channels_pass() {
        let mut engine = Engine::new(48_000.0);
        engine.set_channels(Channels::Swapped);
        engine.set_output_db(-20.0);
        engine.reset();
        let mut left = [1.0_f32; 2];
        let mut right = [0.0_f32; 2];
        let mut third = [0.5_f32; 2];
        engine.process(&mut [&mut left, &mut right, &mut third]);
        assert!(left.iter().all(|sample| close(*sample, 0.0)));
        assert!(right.iter().all(|sample| close(*sample, 0.1)));
        assert_eq!(third, [0.5; 2]);

        // Bypassed, both channels are where they were.
        engine.set_bypass(true);
        engine.reset();
        let mut left = [1.0_f32];
        let mut right = [0.0_f32];
        engine.process(&mut [&mut left, &mut right]);
        assert_eq!((left, right), ([1.0], [0.0]));

        // Mono stays as it is, whatever the stage does.
        let mut engine = Engine::new(48_000.0);
        engine.set_channels(Channels::Mono);
        engine.set_crossfeed(Crossfeed::Strong);
        engine.reset();
        let mut mono = [0.25_f32, -0.5, 1.0];
        engine.process(&mut [&mut mono]);
        assert_eq!(mono, [0.25, -0.5, 1.0]);
    }

    #[test]
    fn the_limiter_comes_after_the_level_and_the_bypass_passes_all() {
        let mut engine = Engine::new(48_000.0);
        engine.set_limiter(true);
        engine.set_output_db(6.0);
        engine.reset();
        let mut left = [0.9_f32; 4];
        let mut right = [-0.45_f32; 4];
        engine.process(&mut [&mut left, &mut right]);
        // 0.9 at +6 dB would be 1.8, 5.4 dB above the ceiling: lowered to -0.3 dBFS, the right channel alike.
        assert!(left.iter().all(|sample| close(*sample, 0.966_05)));
        assert!(right.iter().all(|sample| close(*sample, -0.483_03)));
        assert!((engine.take_reduction_db() - 5.38).abs() < 0.01);
        // The level that leaves AudioRef: the limited one.
        assert!(close(engine.take_peak(), 0.966_05));
        assert!(close(engine.take_peak(), 0.0));

        engine.set_bypass(true);
        engine.reset();
        let mut loud = [1.5_f32; 2];
        engine.process(&mut [&mut loud]);
        assert!(loud.iter().all(|sample| close(*sample, 1.5)));
    }

    /// Where the samples of an impulse through the engine end up: each index with a level above
    /// 0.01, and the level.
    fn impulse_through(engine: &mut Engine, length: usize) -> Vec<(usize, f32)> {
        let mut left = vec![0.0_f32; length];
        let mut right = vec![0.0_f32; length];
        left[0] = 1.0;
        engine.process(&mut [&mut left, &mut right]);
        left.iter()
            .enumerate()
            .filter(|(_, sample)| sample.abs() > 0.01)
            .map(|(index, &sample)| (index, sample))
            .collect()
    }

    #[test]
    fn a_latency_delays_every_path() {
        let mut engine = Engine::new(200.0);
        assert_eq!(engine.latency(), 0);
        engine.set_latency(usize::MAX);
        assert_eq!(engine.latency(), MAX_LATENCY);
        engine.set_latency(3);
        // Without a correction.
        assert_eq!(impulse_through(&mut engine, 8), [(3, 1.0)]);
        // A minimum-phase correction, delayed to the latency.
        drop(engine.set_correction(Some(Box::new(flat(-6.0)))));
        engine.reset();
        let delayed = impulse_through(&mut engine, 8);
        assert_eq!(delayed.len(), 1);
        assert_eq!(delayed[0].0, 3);
        assert!((delayed[0].1 - 0.501).abs() < 1e-3);
        // The bypass too, and a reset empties the delay lines.
        let mut left = [1.0_f32, 0.0];
        let mut right = [0.0_f32; 2];
        engine.process(&mut [&mut left, &mut right]);
        engine.set_bypass(true);
        engine.reset();
        assert_eq!(impulse_through(&mut engine, 8), [(3, 1.0)]);
    }

    #[test]
    fn corrections_of_either_phase_stay_in_line() {
        let eq = ParametricEq::new(-6.0, Vec::new()).expect("accepted");
        let linear = Filter::of_eq(&eq, 48_000.0, Phase::Linear).expect("designed");
        let latency = Phase::Linear.latency(48_000.0);
        assert_eq!(linear.latency(), latency);
        let mut engine = Engine::new(48_000.0);
        engine.set_latency(latency);
        drop(engine.set_correction(Some(Box::new(linear))));
        engine.reset();
        let through = impulse_through(&mut engine, 4096);
        assert_eq!(through.len(), 1);
        assert_eq!(through[0].0, latency);
        // While it fades to the biquads, both come out at the latency: one impulse, still at
        // -6 dB.
        let minimum = Filter::of_eq(&eq, 48_000.0, Phase::Minimum).expect("designed");
        drop(engine.set_correction(Some(Box::new(minimum))));
        let mut left = vec![0.0_f32; 4096];
        let mut right = vec![0.0_f32; 4096];
        left[0] = 1.0;
        engine.process(&mut [&mut left, &mut right]);
        let loud: Vec<usize> = (0..left.len())
            .filter(|&index| left[index].abs() > 0.01)
            .collect();
        assert_eq!(loud, [latency]);
        assert!((left[latency] - 0.501).abs() < 1e-3);
    }
}
