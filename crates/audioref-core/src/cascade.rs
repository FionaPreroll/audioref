//! A correction as the audio thread runs it: the biquads of a [`ParametricEq`] at a sample rate,
//! one after the other, with the state of each channel.
//!
//! The overall gain comes with automatic headroom: if the EQ, with its own overall gain, still
//! boosts some frequency, the gain is lowered by the largest boost, so the correction never makes
//! the signal louder than it was and the master doesn't clip.
//!
//! [`Cascade::new`] allocates, so it runs in the background; [`Cascade::process`] doesn't.
//!
//! ```
//! use audioref_core::cascade::Cascade;
//! use audioref_core::eq::{Band, ParametricEq};
//!
//! let eq = ParametricEq::new(0.0, vec![Band::Peak { frequency: 1000.0, gain_db: 6.0, q: 1.0 }])?;
//! let cascade = Cascade::new(&eq, 48_000.0);
//! assert!((cascade.headroom_db() - 6.0).abs() < 0.01);
//! assert!(cascade.response_db(1000.0).abs() < 0.01);
//! # Ok::<(), audioref_core::eq::EqError>(())
//! ```

use crate::biquad::{self, Coefficients};
use crate::eq::ParametricEq;

/// The number of channels a cascade keeps a state for: left and right.
pub const CHANNELS: usize = 2;

/// The frequencies where the headroom is measured and the curve is drawn, in Hz: AutoEQ's grid,
/// from 20 Hz, each 1.01 times the last, up to 20 kHz.
pub fn grid() -> impl Iterator<Item = f64> {
    std::iter::successors(Some(20.0_f64), |frequency| Some(frequency * 1.01))
        .take_while(|frequency| *frequency <= 20_000.0)
}

/// One sample of a biquad in transposed direct form II, with the last two values of its state.
fn step(c: &Coefficients, state: &mut [f64; 2], input: f64) -> f64 {
    let [s1, s2] = *state;
    let output = c.b0 * input + s1;
    *state = [
        c.b1 * input - c.a1 * output + s2,
        c.b2 * input - c.a2 * output,
    ];
    output
}

/// The biquads of an EQ at a sample rate, with its overall gain and the headroom.
#[derive(Debug, Clone, PartialEq)]
pub struct Cascade {
    sections: Vec<Coefficients>,
    /// The state of each section, for each channel.
    states: [Vec<[f64; 2]>; CHANNELS],
    /// The overall gain with the headroom, as a factor.
    gain: f64,
    headroom_db: f64,
    sample_rate: f64,
}

impl Cascade {
    /// The cascade of an EQ at a sample rate, in Hz. Bands at or above half the sample rate are
    /// left out.
    #[must_use]
    pub fn new(eq: &ParametricEq, sample_rate: f32) -> Self {
        let sample_rate = f64::from(sample_rate);
        let sections: Vec<Coefficients> = eq
            .bands()
            .iter()
            .flat_map(|band| biquad::design(band, sample_rate))
            .collect();
        let mut cascade = Self {
            states: std::array::from_fn(|_| vec![[0.0; 2]; sections.len()]),
            sections,
            gain: 1.0,
            headroom_db: 0.0,
            sample_rate,
        };
        let loudest = grid()
            .filter(|frequency| *frequency < sample_rate / 2.0)
            .map(|frequency| cascade.bands_db(frequency))
            .fold(f64::NEG_INFINITY, f64::max)
            + eq.gain_db();
        cascade.headroom_db = loudest.max(0.0);
        cascade.gain = 10.0_f64.powf((eq.gain_db() - cascade.headroom_db) / 20.0);
        cascade
    }

    /// The sample rate it was designed for, in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// By how much the overall gain was lowered, in dB, so the correction boosts nothing: 0 if it
    /// boosts nothing anyway.
    #[must_use]
    pub fn headroom_db(&self) -> f64 {
        self.headroom_db
    }

    /// The response of the bands alone at a frequency in Hz, in dB.
    fn bands_db(&self, frequency: f64) -> f64 {
        self.sections
            .iter()
            .map(|section| section.response_db(frequency, self.sample_rate))
            .sum()
    }

    /// The response at a frequency in Hz, in dB, with the overall gain and the headroom: the
    /// curve that the window shows.
    #[must_use]
    pub fn response_db(&self, frequency: f64) -> f64 {
        20.0 * self.gain.log10() + self.bands_db(frequency)
    }

    /// Processes one sample of a channel, counted from 0. Channels beyond [`CHANNELS`] pass
    /// unchanged.
    pub fn process(&mut self, channel: usize, sample: f64) -> f64 {
        let Some(states) = self.states.get_mut(channel) else {
            return sample;
        };
        self.sections
            .iter()
            .zip(states.iter_mut())
            .fold(sample * self.gain, |signal, (section, state)| {
                step(section, state, signal)
            })
    }

    /// Forgets what the filters hold, e.g. when playback starts again.
    pub fn reset(&mut self) {
        for state in self.states.iter_mut().flatten() {
            *state = [0.0; 2];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CHANNELS, Cascade, grid};
    use crate::eq::{Band, ParametricEq};

    fn eq(gain_db: f64, bands: Vec<Band>) -> ParametricEq {
        ParametricEq::new(gain_db, bands).expect("accepted")
    }

    fn peak(frequency: f64, gain_db: f64) -> Band {
        Band::Peak {
            frequency,
            gain_db,
            q: 1.0,
        }
    }

    /// The gain of a cascade for a sine of a frequency, measured after it settled.
    fn measured_db(cascade: &mut Cascade, channel: usize, frequency: f64) -> f64 {
        let sample_rate = 48_000.0;
        let samples = 48_000;
        let omega = 2.0 * std::f64::consts::PI * frequency / sample_rate;
        let mut peak = 0.0_f64;
        for n in 0..samples {
            let output = cascade.process(channel, (omega * f64::from(n)).sin());
            if n > samples / 2 {
                peak = peak.max(output.abs());
            }
        }
        20.0 * peak.log10()
    }

    #[test]
    fn the_grid_is_autoeqs() {
        let frequencies: Vec<f64> = grid().collect();
        assert_eq!(frequencies.len(), 695);
        assert!(
            frequencies
                .last()
                .is_some_and(|last| (last - 19_956.0).abs() < 1.0)
        );
    }

    #[test]
    fn processes_what_it_shows() {
        let mut cascade = Cascade::new(
            &eq(-3.0, vec![peak(1000.0, -6.0), peak(5000.0, 2.0)]),
            48_000.0,
        );
        assert!(cascade.headroom_db().abs() < 1e-12);
        for frequency in [100.0, 1000.0, 5000.0] {
            let shown = cascade.response_db(frequency);
            for channel in 0..CHANNELS {
                cascade.reset();
                let measured = measured_db(&mut cascade, channel, frequency);
                assert!(
                    (measured - shown).abs() < 0.05,
                    "{frequency}: {measured} != {shown}"
                );
            }
        }
    }

    #[test]
    fn lowers_the_gain_by_the_largest_boost() {
        // Two boosts of 6 dB, an overall gain of -2 dB: 4 dB of headroom are missing.
        let cascade = Cascade::new(
            &eq(-2.0, vec![peak(100.0, 6.0), peak(8000.0, 6.0)]),
            44_100.0,
        );
        assert!((cascade.headroom_db() - 4.0).abs() < 0.01);
        let loudest = grid()
            .map(|frequency| cascade.response_db(frequency))
            .fold(f64::MIN, f64::max);
        assert!(loudest.abs() < 1e-9, "{loudest}");
    }

    #[test]
    fn leaves_out_bands_above_half_the_sample_rate() {
        // At 32 kHz, the boost at 18 kHz would be above half the sample rate.
        let cascade = Cascade::new(&eq(0.0, vec![peak(18_000.0, 6.0)]), 32_000.0);
        assert!(cascade.headroom_db().abs() < 1e-12);
        assert!(cascade.response_db(1000.0).abs() < 1e-12);
        assert_eq!(cascade.sample_rate().to_bits(), 32_000.0_f64.to_bits());
    }

    #[test]
    fn channels_have_their_own_state_and_others_pass() {
        let mut cascade = Cascade::new(&eq(0.0, vec![peak(1000.0, -12.0)]), 48_000.0);
        let left: Vec<f64> = (0..8).map(|_| cascade.process(0, 1.0)).collect();
        let right: Vec<f64> = (0..8).map(|_| cascade.process(1, 1.0)).collect();
        assert_eq!(left, right);
        assert_eq!(
            cascade.process(CHANNELS, 0.25).to_bits(),
            0.25_f64.to_bits()
        );
        // After a reset, the same input gives the same samples again.
        cascade.reset();
        let again: Vec<f64> = (0..8).map(|_| cascade.process(0, 1.0)).collect();
        assert_eq!(again, left);
    }
}
