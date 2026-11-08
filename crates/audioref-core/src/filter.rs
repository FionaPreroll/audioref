//! A correction as the audio thread runs it: the biquads of an OPRA preset ([`Cascade`]), or the
//! minimum-phase FIR of a correction designed from AutoEQ's curves ([`Fir`]).
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
use crate::fir;

/// The minimum-phase FIR of a correction at a sample rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Fir {
    convolver: Convolver,
    sample_rate: f64,
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
        }))
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
    }
}
