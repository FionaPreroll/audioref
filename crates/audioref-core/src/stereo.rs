//! The stereo stage: the channels as they are, in mono or swapped, and crossfeed, which lets each
//! ear hear some of the other channel's bass, as from speakers, so wide mixes tire less on
//! headphones.
//!
//! The crossfeed follows Bauer's idea, as bs2b does, in a form that leaves the middle alone: a
//! first-order lowpass at [`CROSSFEED_HZ`] of the difference of the channels, taken from the left
//! and added to the right, `L' = L − g·LP(L − R)` and `R' = R + g·LP(L − R)`. What both channels
//! share passes unchanged, and below the lowpass each channel gets the other's bass
//! [`Crossfeed::level_db`] below its own. A change of either setting fades over
//! [`RAMP_SECONDS`](crate::gain::RAMP_SECONDS), so it doesn't click.
//!
//! ```
//! use audioref_core::stereo::{Channels, Crossfeed, Stereo};
//!
//! let mut stereo = Stereo::new(48_000.0);
//! stereo.set_channels(Channels::Swapped);
//! stereo.reset();
//! assert_eq!(stereo.process(1.0, 0.25), (0.25, 1.0));
//!
//! stereo.set_channels(Channels::Stereo);
//! stereo.set_crossfeed(Crossfeed::Strong);
//! stereo.reset();
//! // The bass of the left channel alone: the right ear hears it 4.5 dB lower.
//! let (left, right) = (0..48_000).fold((0.0, 0.0), |_, _| stereo.process(1.0, 0.0));
//! assert!((20.0 * (left / right).log10() - 4.5).abs() < 1e-4);
//! ```

use crate::gain::{Ramp, ramp_samples};

/// The corner of the crossfeed's lowpass, in Hz.
pub const CROSSFEED_HZ: f64 = 700.0;

/// How the two channels reach the ears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Channels {
    /// As they are.
    #[default]
    Stereo,
    /// Both channels in both ears, at half the level each, to check a mix in mono.
    Mono,
    /// The left channel in the right ear and the other way round, e.g. to check headphones.
    Swapped,
}

impl Channels {
    /// All of them, in the order of their indices.
    pub const ALL: [Self; 3] = [Self::Stereo, Self::Mono, Self::Swapped];

    /// The index of the host's parameter, from 0.
    #[must_use]
    pub fn index(self) -> i32 {
        match self {
            Self::Stereo => 0,
            Self::Mono => 1,
            Self::Swapped => 2,
        }
    }

    /// The setting of an index of the host's parameter; stereo for one out of range.
    #[must_use]
    pub fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Mono,
            2 => Self::Swapped,
            _ => Self::Stereo,
        }
    }

    /// How much of the left and the right input each output takes: `[[ll, lr], [rl, rr]]`.
    fn matrix(self) -> [[f32; 2]; 2] {
        match self {
            Self::Stereo => [[1.0, 0.0], [0.0, 1.0]],
            Self::Mono => [[0.5, 0.5], [0.5, 0.5]],
            Self::Swapped => [[0.0, 1.0], [1.0, 0.0]],
        }
    }
}

/// How much of each channel's bass the other ear hears, in bs2b's three strengths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Crossfeed {
    /// None.
    #[default]
    Off,
    /// 9.5 dB below the channel itself, as Jan Meier's.
    Light,
    /// 6 dB below, as Chu Moy's.
    Medium,
    /// 4.5 dB below, bs2b's default.
    Strong,
}

impl Crossfeed {
    /// All of them, in the order of their indices.
    pub const ALL: [Self; 4] = [Self::Off, Self::Light, Self::Medium, Self::Strong];

    /// The index of the host's parameter, from 0.
    #[must_use]
    pub fn index(self) -> i32 {
        match self {
            Self::Off => 0,
            Self::Light => 1,
            Self::Medium => 2,
            Self::Strong => 3,
        }
    }

    /// The strength of an index of the host's parameter; off for one out of range.
    #[must_use]
    pub fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Light,
            2 => Self::Medium,
            3 => Self::Strong,
            _ => Self::Off,
        }
    }

    /// How far below a channel's own bass the other ear hears it, in dB; none when off.
    #[must_use]
    pub fn level_db(self) -> Option<f64> {
        match self {
            Self::Off => None,
            Self::Light => Some(9.5),
            Self::Medium => Some(6.0),
            Self::Strong => Some(4.5),
        }
    }

    /// The factor `g` of the difference, so that `g / (1 − g)` is the level below.
    fn share(self) -> f32 {
        self.level_db().map_or(0.0, |db| {
            let ratio = 10.0_f64.powf(-db / 20.0);
            #[expect(clippy::cast_possible_truncation, reason = "a factor below 1")]
            let share = (ratio / (1.0 + ratio)) as f32;
            share
        })
    }
}

/// The stereo stage, with the state of its lowpass and its fades.
#[derive(Debug, Clone, PartialEq)]
pub struct Stereo {
    /// The matrix of [`Channels`], each factor faded on its own.
    matrix: [[Ramp; 2]; 2],
    /// The factor of the crossfeed.
    share: Ramp,
    /// The coefficient of the lowpass at the sample rate.
    coefficient: f64,
    /// The lowpassed difference of the channels.
    difference: f64,
}

impl Stereo {
    /// The channels as they are, without crossfeed, for a sample rate in Hz.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let length = ramp_samples(sample_rate);
        let matrix = Channels::Stereo
            .matrix()
            .map(|row| row.map(|factor| Ramp::new(factor, length)));
        Self {
            matrix,
            share: Ramp::new(0.0, length),
            coefficient: coefficient(sample_rate),
            difference: 0.0,
        }
    }

    /// Adapts the fades and the lowpass to a new sample rate in Hz.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let length = ramp_samples(sample_rate);
        for ramp in self.matrix.iter_mut().flatten() {
            ramp.set_length(length);
        }
        self.share.set_length(length);
        self.coefficient = coefficient(sample_rate);
    }

    /// Fades to a way of routing the channels.
    pub fn set_channels(&mut self, channels: Channels) {
        for (ramps, factors) in self.matrix.iter_mut().zip(channels.matrix()) {
            for (ramp, factor) in ramps.iter_mut().zip(factors) {
                ramp.set_target(factor);
            }
        }
    }

    /// Fades to a strength of the crossfeed.
    pub fn set_crossfeed(&mut self, crossfeed: Crossfeed) {
        self.share.set_target(crossfeed.share());
    }

    /// Ends the fades at once and empties the lowpass, e.g. when playback starts again.
    pub fn reset(&mut self) {
        for ramp in self.matrix.iter_mut().flatten() {
            ramp.finish();
        }
        self.share.finish();
        self.difference = 0.0;
    }

    /// Processes one sample of each channel.
    pub fn process(&mut self, left: f64, right: f64) -> (f64, f64) {
        let [[ll, lr], [rl, rr]] = self
            .matrix
            .each_mut()
            .map(|row| row.each_mut().map(|ramp| f64::from(ramp.next_value())));
        let routed_left = ll * left + lr * right;
        let routed_right = rl * left + rr * right;
        let share = f64::from(self.share.next_value());
        // Without crossfeed too, so the lowpass is ready when it fades in.
        self.difference += self.coefficient * (routed_left - routed_right - self.difference);
        let fed = share * self.difference;
        (routed_left - fed, routed_right + fed)
    }
}

/// The coefficient of a one-pole lowpass at [`CROSSFEED_HZ`]: 1 for a sample rate it doesn't
/// fit, which passes the difference unfiltered.
fn coefficient(sample_rate: f32) -> f64 {
    let coefficient = 1.0 - (-std::f64::consts::TAU * CROSSFEED_HZ / f64::from(sample_rate)).exp();
    if coefficient.is_finite() && coefficient > 0.0 {
        coefficient.min(1.0)
    } else {
        1.0
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "the routed samples are exact multiples of the input"
)]
mod tests {
    use super::*;

    /// The levels of both outputs after a second of a constant input, so the lowpass settled.
    fn settled(stereo: &mut Stereo, left: f64, right: f64) -> (f64, f64) {
        (0..48_000).fold((0.0, 0.0), |_, _| stereo.process(left, right))
    }

    #[test]
    fn routes_the_channels() {
        let mut stereo = Stereo::new(48_000.0);
        assert_eq!(stereo.process(0.5, -0.25), (0.5, -0.25));
        stereo.set_channels(Channels::Mono);
        stereo.reset();
        assert_eq!(stereo.process(0.5, -0.25), (0.125, 0.125));
        stereo.set_channels(Channels::Swapped);
        stereo.reset();
        assert_eq!(stereo.process(0.5, -0.25), (-0.25, 0.5));
    }

    #[test]
    fn indices_of_the_hosts_parameters() {
        for (index, channels) in (0..).zip(Channels::ALL) {
            assert_eq!(channels.index(), index);
            assert_eq!(Channels::from_index(index), channels);
        }
        for (index, crossfeed) in (0..).zip(Crossfeed::ALL) {
            assert_eq!(crossfeed.index(), index);
            assert_eq!(Crossfeed::from_index(index), crossfeed);
        }
        assert_eq!(Channels::from_index(-1), Channels::Stereo);
        assert_eq!(Channels::from_index(3), Channels::Stereo);
        assert_eq!(Crossfeed::from_index(4), Crossfeed::Off);
        assert_eq!(Channels::default(), Channels::Stereo);
        assert_eq!(Crossfeed::default(), Crossfeed::Off);
    }

    #[test]
    fn a_change_fades() {
        let mut stereo = Stereo::new(200.0); // 4 samples per fade
        stereo.set_channels(Channels::Swapped);
        let left: Vec<f64> = (0..5).map(|_| stereo.process(1.0, 0.0).0).collect();
        assert_eq!(left, [0.75, 0.5, 0.25, 0.0, 0.0]);
        // Faster at a higher rate.
        stereo.set_sample_rate(100.0);
        stereo.set_channels(Channels::Stereo);
        let left: Vec<f64> = (0..3).map(|_| stereo.process(1.0, 0.0).0).collect();
        assert_eq!(left, [0.5, 1.0, 1.0]);
    }

    #[test]
    fn crossfeed_moves_the_bass_of_each_channel_into_the_other_ear() {
        for crossfeed in [Crossfeed::Light, Crossfeed::Medium, Crossfeed::Strong] {
            let mut stereo = Stereo::new(48_000.0);
            stereo.set_crossfeed(crossfeed);
            stereo.reset();
            let (left, right) = settled(&mut stereo, 1.0, 0.0);
            let below = 20.0 * (left / right).log10();
            assert!((below - crossfeed.level_db().unwrap()).abs() < 1e-4);
            // The other way round alike.
            let (left, right) = settled(&mut stereo, 0.0, 1.0);
            assert!((20.0 * (right / left).log10() - below).abs() < 1e-6);
        }
        assert_eq!(Crossfeed::Off.level_db(), None);
    }

    #[test]
    fn crossfeed_leaves_the_middle_and_the_treble_alone() {
        let mut stereo = Stereo::new(48_000.0);
        stereo.set_crossfeed(Crossfeed::Strong);
        stereo.reset();
        // What both channels share passes unchanged.
        for n in 0..1000 {
            let x = (f64::from(n) * 0.1).sin();
            assert_eq!(stereo.process(x, x), (x, x));
        }
        // At half the sample rate, the other ear gets little.
        let mut right = 0.0_f64;
        for n in 0..4800 {
            let x = if n % 2 == 0 { 1.0 } else { -1.0 };
            let fed = stereo.process(x, 0.0).1.abs();
            // Once the lowpass settled.
            if n >= 100 {
                right = right.max(fed);
            }
        }
        assert!(right < 0.02, "{right}");
    }

    #[test]
    fn a_reset_empties_the_lowpass() {
        let mut stereo = Stereo::new(48_000.0);
        stereo.set_crossfeed(Crossfeed::Medium);
        let fresh = {
            let mut fresh = stereo.clone();
            fresh.reset();
            fresh
        };
        settled(&mut stereo, 1.0, 0.0);
        assert_ne!(stereo, fresh);
        stereo.reset();
        assert_eq!(stereo, fresh);
    }

    #[test]
    fn sample_rates_the_lowpass_doesnt_fit() {
        for sample_rate in [0.0, -1.0, f32::NAN, 1.0, f32::INFINITY] {
            assert_eq!(coefficient(sample_rate), 1.0, "{sample_rate}");
        }
        let at_48k = coefficient(48_000.0);
        assert!(at_48k > 0.08 && at_48k < 0.09);
    }
}
