//! A correction as the audio thread runs it: the biquads of an OPRA preset ([`Cascade`]), or the
//! minimum-phase FIR of a correction designed from AutoEQ's curves ([`Fir`]). With
//! [`Phase::Linear`], both become linear-phase FIRs instead, which shift no phase but delay the
//! signal by their [`Filter::latency`].
//!
//! Both are made in the background, where they allocate, and processed sample by sample in the
//! audio thread, where they don't.
//!
//! ```
//! use audioref_core::design::Correction;
//! use audioref_core::filter::Filter;
//!
//! let flat = Correction::from_levels(vec![0.0; 695]).ok_or("not a correction")?;
//! let mut filter = Filter::fir(&flat, 48_000.0).ok_or("no filter")?;
//! assert_eq!(filter.sample_rate(), 48_000.0);
//! assert!((filter.process(0, 0.5) - 0.5).abs() < 1e-3);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::cascade::Cascade;
use crate::convolver::Convolver;
use crate::design::Correction;
use crate::eq::ParametricEq;
use crate::fir;

/// How a correction treats the phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    /// The biquads of a preset or the minimum-phase FIR of a design: no latency, but the phase
    /// shifts where the level changes, as with analog EQs.
    #[default]
    Minimum,
    /// A linear-phase FIR: the phase stays, but the signal is delayed by [`fir::latency`].
    Linear,
}

impl Phase {
    /// Linear if `linear`, else minimum.
    #[must_use]
    pub fn from_linear(linear: bool) -> Self {
        if linear { Self::Linear } else { Self::Minimum }
    }

    /// The latency of a correction of this phase at a sample rate in Hz, in samples.
    #[must_use]
    pub fn latency(self, sample_rate: f32) -> usize {
        match self {
            Self::Minimum => 0,
            Self::Linear => fir::latency(f64::from(sample_rate)),
        }
    }

    /// The latency of a correction of this phase at a sample rate in Hz, in milliseconds.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "latencies of a few thousand samples"
    )]
    pub fn latency_ms(self, sample_rate: f32) -> f64 {
        self.latency(sample_rate) as f64 / f64::from(sample_rate) * 1000.0
    }
}

/// The FIR of a correction at a sample rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Fir {
    convolver: Convolver,
    sample_rate: f64,
    /// By how much it delays the signal, in samples: 0 for a minimum-phase filter.
    latency: usize,
}

/// A correction as the audio thread runs it.
#[derive(Debug, Clone, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "filters travel in a Box anyway, see crate::engine"
)]
pub enum Filter {
    /// The biquads of a parametric EQ.
    Biquads(Cascade),
    /// The FIR of a designed correction.
    Fir(Fir),
}

impl From<Cascade> for Filter {
    fn from(cascade: Cascade) -> Self {
        Self::Biquads(cascade)
    }
}

impl Filter {
    /// The FIR of a correction at a sample rate in Hz, see [`fir::minimum_phase`].
    #[must_use]
    pub fn fir(correction: &Correction, sample_rate: f32) -> Option<Self> {
        let sample_rate = f64::from(sample_rate);
        let taps = fir::minimum_phase(correction, sample_rate)?;
        Some(Self::Fir(Fir {
            convolver: Convolver::new(&taps),
            sample_rate,
            latency: 0,
        }))
    }

    /// The linear-phase FIR of a response in dB at a sample rate in Hz, see
    /// [`fir::linear_phase`].
    #[must_use]
    pub fn linear(response_db: &dyn Fn(f64) -> f64, sample_rate: f32) -> Option<Self> {
        let sample_rate = f64::from(sample_rate);
        let taps = fir::linear_phase(response_db, sample_rate)?;
        Some(Self::Fir(Fir {
            convolver: Convolver::new(&taps),
            sample_rate,
            latency: fir::latency(sample_rate),
        }))
    }

    /// The filter of a designed correction, lowered by its headroom, at a sample rate in Hz.
    #[must_use]
    pub fn of_correction(correction: &Correction, sample_rate: f32, phase: Phase) -> Option<Self> {
        match phase {
            Phase::Minimum => Self::fir(correction, sample_rate),
            Phase::Linear => {
                let headroom_db = correction.headroom_db();
                Self::linear(
                    &|frequency| correction.response_db(frequency) - headroom_db,
                    sample_rate,
                )
            }
        }
    }

    /// The filter of a preset, lowered by its headroom, at a sample rate in Hz: its biquads, or a
    /// linear-phase FIR of the same level at every frequency.
    #[must_use]
    pub fn of_eq(eq: &ParametricEq, sample_rate: f32, phase: Phase) -> Option<Self> {
        let cascade = Cascade::new(eq, sample_rate);
        match phase {
            Phase::Minimum => Some(Self::from(cascade)),
            Phase::Linear => Self::linear(&|frequency| cascade.response_db(frequency), sample_rate),
        }
    }

    /// By how much it delays the signal, in samples.
    #[must_use]
    pub fn latency(&self) -> usize {
        match self {
            Self::Biquads(_) => 0,
            Self::Fir(fir) => fir.latency,
        }
    }

    /// The sample rate it was designed for, in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> f64 {
        match self {
            Self::Biquads(cascade) => cascade.sample_rate(),
            Self::Fir(fir) => fir.sample_rate,
        }
    }

    /// Processes one sample of a channel, counted from 0. Channels beyond
    /// [`crate::cascade::CHANNELS`] pass unchanged.
    pub fn process(&mut self, channel: usize, sample: f64) -> f64 {
        match self {
            Self::Biquads(cascade) => cascade.process(channel, sample),
            Self::Fir(fir) => fir.convolver.process(channel, sample),
        }
    }

    /// Empties the state of each channel.
    pub fn reset(&mut self) {
        match self {
            Self::Biquads(cascade) => cascade.reset(),
            Self::Fir(fir) => fir.convolver.reset(),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "sample rates and samples taken over unchanged"
)]
mod tests {
    use super::*;
    use crate::eq::{Band, ParametricEq};

    #[test]
    fn biquads_and_fir_alike() {
        let eq = ParametricEq::new(
            -2.0,
            vec![Band::Peak {
                frequency: 1000.0,
                gain_db: 3.0,
                q: 1.0,
            }],
        )
        .unwrap();
        let cascade = Cascade::new(&eq, 44_100.0);
        let mut biquads = Filter::from(cascade.clone());
        assert_eq!(biquads.sample_rate(), 44_100.0);
        let mut alone = cascade;
        assert_eq!(biquads.process(0, 1.0), alone.process(0, 1.0));
        biquads.reset();
        alone.reset();
        assert_eq!(biquads, Filter::Biquads(alone));

        let flat = Correction::from_levels(vec![-1.0; 695]).unwrap();
        let mut fir = Filter::fir(&flat, 96_000.0).unwrap();
        assert_eq!(fir.sample_rate(), 96_000.0);
        let fresh = fir.clone();
        // -1 dB everywhere: no headroom, an impulse at -1 dB.
        assert!((fir.process(1, 1.0) - 0.891).abs() < 1e-3);
        fir.reset();
        assert_eq!(fir, fresh);
        assert_eq!(fir.latency(), 0);
    }

    #[test]
    fn phases_and_their_latencies() {
        assert_eq!(Phase::default(), Phase::Minimum);
        assert_eq!(Phase::from_linear(false), Phase::Minimum);
        assert_eq!(Phase::from_linear(true), Phase::Linear);
        assert_eq!(Phase::Minimum.latency(48_000.0), 0);
        assert_eq!(Phase::Linear.latency(48_000.0), fir::latency(48_000.0));
        assert_eq!(Phase::Minimum.latency_ms(44_100.0), 0.0);
        assert!((Phase::Linear.latency_ms(48_000.0) - 42.6).abs() < 0.1);
        assert!((Phase::Linear.latency_ms(192_000.0) - 42.7).abs() < 0.1);
    }

    /// The first sample far from 0 of a filter's impulse response, and its value.
    fn first_sample(filter: &mut Filter) -> (usize, f64) {
        (0..20_000)
            .map(|index| (index, filter.process(0, if index == 0 { 1.0 } else { 0.0 })))
            .find(|(_, sample)| sample.abs() > 0.1)
            .unwrap()
    }

    #[test]
    fn of_a_preset_and_a_design_in_both_phases() {
        let eq = ParametricEq::new(-6.0, Vec::new()).unwrap();
        let mut biquads = Filter::of_eq(&eq, 48_000.0, Phase::Minimum).unwrap();
        assert!(matches!(biquads, Filter::Biquads(_)));
        assert_eq!(biquads.latency(), 0);
        assert_eq!(first_sample(&mut biquads).0, 0);
        let mut linear = Filter::of_eq(&eq, 48_000.0, Phase::Linear).unwrap();
        assert_eq!(linear.latency(), fir::latency(48_000.0));
        assert_eq!(linear.sample_rate(), 48_000.0);
        // Flat at -6 dB: an impulse at half the level, at the latency.
        let (index, value) = first_sample(&mut linear);
        assert_eq!(index, linear.latency());
        assert!((value - 0.501).abs() < 1e-3);

        // +2 dB everywhere: lowered by its headroom to 0 dB.
        let boost = Correction::from_levels(vec![2.0; 695]).unwrap();
        let mut minimum = Filter::of_correction(&boost, 44_100.0, Phase::Minimum).unwrap();
        assert_eq!(minimum.latency(), 0);
        assert!((first_sample(&mut minimum).1 - 1.0).abs() < 1e-2);
        let mut linear = Filter::of_correction(&boost, 44_100.0, Phase::Linear).unwrap();
        assert_eq!(linear.latency(), fir::latency(44_100.0));
        let (index, value) = first_sample(&mut linear);
        assert_eq!(index, linear.latency());
        assert!((value - 1.0).abs() < 1e-2);
    }
}
