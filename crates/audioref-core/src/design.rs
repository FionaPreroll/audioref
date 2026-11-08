//! The design of a correction from a measurement of headphones and a target: the levels, in dB,
//! that bring the measurement to the target, on AutoEQ's grid from 20 Hz to 20 kHz
//! ([`crate::cascade::grid`]).
//!
//! 1. Correction = target + tilt + bass shelf − measurement.
//! 2. Smoothed: over 1/12 octave up to 2 kHz, wider above, 1/3 octave from 8 kHz, so narrow
//!    peaks and dips of a single measurement, which move with the fit of the headphones, aren't
//!    copied.
//! 3. Set to 0 dB on average from 200 Hz to 2 kHz, where the ear is most sensitive to level.
//! 4. Faded out from 12 to 16 kHz, nothing above: measurements up there depend on the ear and the
//!    fit more than on the headphones.
//! 5. Boosts limited, by default to +6 dB; cuts to −30 dB.
//!
//! [`crate::fir`] turns it into a filter; the overall gain is lowered by the largest boost,
//! [`Correction::headroom_db`], so it boosts nothing.
//!
//! ```
//! use audioref_core::autoeq::Curve;
//! use audioref_core::design::{Correction, Settings};
//!
//! let measurement = Curve::from_points(vec![(20.0, -6.0), (1000.0, 0.0), (20_000.0, 0.0)])
//!     .map_err(|error| format!("{error:?}"))?;
//! let target = Curve::from_points(vec![(20.0, 0.0), (20_000.0, 0.0)])
//!     .map_err(|error| format!("{error:?}"))?;
//! let correction = Correction::design(&measurement, &target, Settings::default());
//! // The bass is boosted, by at most 6 dB.
//! assert!(correction.response_db(20.0) > 5.0 && correction.response_db(20.0) <= 6.0);
//! assert!(correction.response_db(1000.0).abs() < 0.1);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::autoeq::Curve;
use crate::biquad;
use crate::cascade::grid;
use crate::eq::Band;

/// The tilt, in dB per octave, around 1 kHz: positive brightens, negative darkens.
pub const TILT_DB_PER_OCTAVE: RangeInclusive<f64> = -1.0..=1.0;

/// The gain of the bass shelf, in dB.
pub const BASS_DB: RangeInclusive<f64> = -6.0..=12.0;

/// Where the bass shelf has half its gain, in Hz, and its Q: those of the Harman target's shelf.
pub const BASS_SHELF: (f64, f64) = (105.0, 0.71);

/// The most a correction may boost, in dB.
pub const MAX_BOOST_DB: RangeInclusive<f64> = 0.0..=12.0;

/// The most a correction cuts, in dB.
pub const MAX_CUT_DB: f64 = -30.0;

/// Where the correction fades out, in Hz: fully up to the start, nothing from the end.
pub const FADE_OUT_HZ: RangeInclusive<f64> = 12_000.0..=16_000.0;

/// Where the correction is set to 0 dB on average, in Hz.
pub const LEVEL_HZ: RangeInclusive<f64> = 200.0..=2_000.0;

/// The step of the grid: each frequency is 1.01 times the last.
const STEP: f64 = 1.01;

/// What users set beyond the measurement and the target.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// The tilt, in dB per octave around 1 kHz, see [`TILT_DB_PER_OCTAVE`].
    pub tilt_db_per_octave: f64,
    /// The gain of the bass shelf, in dB, see [`BASS_DB`] and [`BASS_SHELF`].
    pub bass_db: f64,
    /// The most the correction may boost, in dB, see [`MAX_BOOST_DB`].
    pub max_boost_db: f64,
}

impl Default for Settings {
    /// No tilt, no bass shelf, boosts up to +6 dB.
    fn default() -> Self {
        Self {
            tilt_db_per_octave: 0.0,
            bass_db: 0.0,
            max_boost_db: 6.0,
        }
    }
}

/// A value within a range; the default if it isn't a number.
fn clamp(value: f64, range: &RangeInclusive<f64>, default: f64) -> f64 {
    if value.is_nan() {
        default
    } else {
        value.clamp(*range.start(), *range.end())
    }
}

impl Settings {
    /// The settings with each value within its range, e.g. after a project was loaded.
    #[must_use]
    pub fn clamped(self) -> Self {
        let default = Self::default();
        Self {
            tilt_db_per_octave: clamp(
                self.tilt_db_per_octave,
                &TILT_DB_PER_OCTAVE,
                default.tilt_db_per_octave,
            ),
            bass_db: clamp(self.bass_db, &BASS_DB, default.bass_db),
            max_boost_db: clamp(self.max_boost_db, &MAX_BOOST_DB, default.max_boost_db),
        }
    }

    /// The tilt and the bass shelf at a frequency in Hz, in dB.
    fn shape_db(&self, frequency: f64, bass: &[biquad::Coefficients]) -> f64 {
        self.tilt_db_per_octave * (frequency / 1000.0).log2()
            + bass
                .iter()
                .map(|section| section.response_db(frequency, SHELF_SAMPLE_RATE))
                .sum::<f64>()
    }
}

/// The sample rate the bass shelf is computed at, in Hz: high enough for it to match the analog
/// shelf up to 20 kHz.
const SHELF_SAMPLE_RATE: f64 = 384_000.0;

/// The width to smooth over at a frequency in Hz, in octaves.
fn smoothing_octaves(frequency: f64) -> f64 {
    const NARROW: f64 = 1.0 / 12.0;
    const WIDE: f64 = 1.0 / 3.0;
    let share = ((frequency / 2_000.0).log2() / 2.0).clamp(0.0, 1.0);
    NARROW + share * (WIDE - NARROW)
}

/// How much of the correction is kept at a frequency in Hz, from 1 to 0, see [`FADE_OUT_HZ`].
fn fade(frequency: f64) -> f64 {
    let (start, end) = (*FADE_OUT_HZ.start(), *FADE_OUT_HZ.end());
    let share = ((frequency / start).ln() / (end / start).ln()).clamp(0.0, 1.0);
    0.5 + 0.5 * (std::f64::consts::PI * share).cos()
}

/// A correction: a level in dB at each frequency of the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Correction {
    levels_db: Vec<f64>,
}

impl Correction {
    /// The correction that brings a measurement to a target, with the settings clamped to their
    /// ranges.
    #[must_use]
    pub fn design(measurement: &Curve, target: &Curve, settings: Settings) -> Self {
        let settings = settings.clamped();
        let (frequency, gain_db, q) = (BASS_SHELF.0, settings.bass_db, BASS_SHELF.1);
        let bass = biquad::design(
            &Band::LowShelf {
                frequency,
                gain_db,
                q,
            },
            SHELF_SAMPLE_RATE,
        );
        let frequencies: Vec<f64> = grid().collect();
        let raw: Vec<f64> = frequencies
            .iter()
            .map(|&f| target.value_db(f) + settings.shape_db(f, &bass) - measurement.value_db(f))
            .collect();
        let smoothed: Vec<f64> = frequencies
            .iter()
            .enumerate()
            .map(|(index, &f)| {
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a few dozen points at most, and never negative"
                )]
                let half = (smoothing_octaves(f) / 2.0 / STEP.log2()).round() as usize;
                let start = index.saturating_sub(half);
                let window = raw.iter().skip(start).take(index + half + 1 - start);
                let (sum, count) =
                    window.fold((0.0, 0.0), |(sum, count), level| (sum + level, count + 1.0));
                sum / count
            })
            .collect();
        let (sum, count) = frequencies
            .iter()
            .zip(&smoothed)
            .filter(|(f, _)| LEVEL_HZ.contains(f))
            .fold((0.0, 0.0), |(sum, count), (_, level)| {
                (sum + level, count + 1.0)
            });
        let offset = sum / count;
        let levels_db = frequencies
            .iter()
            .zip(&smoothed)
            .map(|(&f, level)| {
                ((level - offset) * fade(f)).clamp(MAX_CUT_DB, settings.max_boost_db)
            })
            .collect();
        Self { levels_db }
    }

    /// A correction from its levels on the grid, as a project saved it: `None` unless there is
    /// one for each frequency of the grid, and each is a number from [`MAX_CUT_DB`] to the end of
    /// [`MAX_BOOST_DB`].
    #[must_use]
    pub fn from_levels(levels_db: Vec<f64>) -> Option<Self> {
        let valid = levels_db.len() == grid().count()
            && levels_db
                .iter()
                .all(|level| (MAX_CUT_DB..=*MAX_BOOST_DB.end()).contains(level));
        valid.then_some(Self { levels_db })
    }

    /// The levels at the frequencies of the grid, in dB.
    #[must_use]
    pub fn levels_db(&self) -> &[f64] {
        &self.levels_db
    }

    /// The level at a frequency in Hz, in dB: between the frequencies of the grid straight on a
    /// logarithmic frequency axis, below 20 Hz the level at 20 Hz, above the grid that of its
    /// last frequency.
    #[must_use]
    pub fn response_db(&self, frequency: f64) -> f64 {
        #[allow(clippy::cast_precision_loss, reason = "695 levels")]
        let last = self.levels_db.len().saturating_sub(1) as f64;
        let position = ((frequency / 20.0).ln() / STEP.ln()).clamp(0.0, last);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "from 0 to the last index"
        )]
        let index = position.floor() as usize;
        let share = position - position.floor();
        let at = |index: usize| {
            self.levels_db
                .get(index)
                .or(self.levels_db.last())
                .copied()
                .unwrap_or(0.0)
        };
        at(index) + share * (at(index.saturating_add(1)) - at(index))
    }

    /// By how much the overall gain is lowered, in dB, so the correction boosts nothing: its
    /// largest boost, 0 if it boosts nothing.
    #[must_use]
    pub fn headroom_db(&self) -> f64 {
        self.levels_db.iter().copied().fold(0.0, f64::max)
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "levels clamped to a limit, or taken over unchanged, are exact"
)]
mod tests {
    use super::*;

    fn curve(level: impl Fn(f64) -> f64) -> Curve {
        Curve::from_points(grid().map(|f| (f, level(f))).collect()).unwrap()
    }

    fn flat() -> Curve {
        curve(|_| 0.0)
    }

    #[test]
    fn a_measurement_on_target_needs_no_correction() {
        let measurement = curve(|f| (f / 1000.0).log2());
        let correction = Correction::design(&measurement, &measurement, Settings::default());
        assert!(
            correction
                .levels_db()
                .iter()
                .all(|level| level.abs() < 1e-9)
        );
        assert!(correction.headroom_db() < 1e-9);
    }

    #[test]
    fn corrects_toward_the_target_and_levels_the_middle() {
        let measurement = curve(|f| if f < 150.0 { -4.0 } else { 3.0 });
        let settings = Settings {
            max_boost_db: 12.0,
            ..Settings::default()
        };
        let correction = Correction::design(&measurement, &flat(), settings);
        assert!((correction.response_db(50.0) - 7.0).abs() < 0.3);
        assert!(correction.response_db(1000.0).abs() < 0.3);
        assert!((correction.headroom_db() - correction.response_db(50.0)).abs() < 1e-9);
    }

    #[test]
    fn limits_boosts_and_cuts() {
        let measurement = curve(|f| {
            if f < 100.0 {
                -20.0
            } else if f > 4000.0 && f < 8000.0 {
                50.0
            } else {
                0.0
            }
        });
        let correction = Correction::design(&measurement, &flat(), Settings::default());
        assert!((correction.response_db(30.0) - 6.0).abs() < 1e-9);
        assert!((correction.response_db(6000.0) - MAX_CUT_DB).abs() < 1e-9);
        let settings = Settings {
            max_boost_db: 0.0,
            ..Settings::default()
        };
        let correction = Correction::design(&measurement, &flat(), settings);
        assert_eq!(correction.headroom_db(), 0.0);
    }

    #[test]
    fn smooths_narrow_dips() {
        let measurement = curve(|f| {
            if (990.0..1010.0).contains(&f) {
                -10.0
            } else {
                0.0
            }
        });
        let correction = Correction::design(&measurement, &flat(), Settings::default());
        // A dip of 1/70 octave spread over 1/12 octave: less than a third of it is left.
        assert!(correction.response_db(1000.0) < 3.0);
        let wide = curve(|f| {
            if (8000.0..8100.0).contains(&f) {
                -10.0
            } else {
                0.0
            }
        });
        let correction = Correction::design(&wide, &flat(), Settings::default());
        assert!(correction.response_db(8050.0) < 0.5);
    }

    #[test]
    fn fades_out_at_the_top() {
        let measurement = curve(|_| -3.0);
        let target = curve(|f| if f > 5000.0 { 4.0 } else { 0.0 });
        let correction = Correction::design(&measurement, &target, Settings::default());
        assert!((correction.response_db(10_000.0) - 4.0).abs() < 1e-6);
        assert!((correction.response_db(13_900.0) - 2.0).abs() < 0.1);
        assert!(correction.response_db(16_000.0).abs() < 0.01);
        assert_eq!(correction.response_db(19_000.0), 0.0);
        assert_eq!(correction.response_db(40_000.0), 0.0);
    }

    #[test]
    fn tilt_and_bass() {
        let settings = Settings {
            tilt_db_per_octave: -0.5,
            bass_db: 6.0,
            max_boost_db: 12.0,
        };
        let correction = Correction::design(&flat(), &flat(), settings);
        // The tilt over 3 octaves, and a little of the shelf that is left at 250 Hz.
        let at = |f| correction.response_db(f);
        assert!((at(250.0) - at(2000.0) - 1.5).abs() < 0.3);
        assert!((at(20.0) - at(250.0) - 6.0 - 0.5 * (250.0_f64 / 20.0).log2()).abs() < 0.3);
        assert!((at(105.0) - at(250.0) - 3.0).abs() < 1.0);
    }

    #[test]
    fn clamps_settings() {
        let settings = Settings {
            tilt_db_per_octave: 5.0,
            bass_db: f64::NAN,
            max_boost_db: -1.0,
        };
        assert_eq!(
            settings.clamped(),
            Settings {
                tilt_db_per_octave: 1.0,
                bass_db: 0.0,
                max_boost_db: 0.0
            }
        );
        let json = serde_json::to_string(&Settings::default()).unwrap();
        assert_eq!(
            serde_json::from_str::<Settings>(&json).unwrap(),
            Settings::default()
        );
    }

    #[test]
    fn from_levels_checks_them() {
        let levels: Vec<f64> = grid().map(|_| 1.0).collect();
        let correction = Correction::from_levels(levels.clone()).unwrap();
        assert_eq!(correction.levels_db(), levels.as_slice());
        assert!(Correction::from_levels(vec![1.0; 10]).is_none());
        let mut bad = levels;
        bad[3] = f64::NAN;
        assert!(Correction::from_levels(bad.clone()).is_none());
        bad[3] = 13.0;
        assert!(Correction::from_levels(bad).is_none());
    }

    #[test]
    fn response_between_and_beyond_the_grid() {
        let levels: Vec<f64> = grid().map(|f| (f / 1000.0).log2()).collect();
        let correction = Correction::from_levels(levels).unwrap();
        assert!((correction.response_db(2000.0) - 1.0).abs() < 1e-6);
        assert!((correction.response_db(1414.2) - 0.5).abs() < 1e-3);
        assert!((correction.response_db(1.0) - (0.02_f64).log2()).abs() < 1e-9);
        let last = *correction.levels_db().last().unwrap();
        assert_eq!(correction.response_db(1e9), last);
        assert_eq!(correction.response_db(f64::INFINITY), last);
        let empty = Correction { levels_db: vec![] };
        assert_eq!(empty.response_db(100.0), 0.0);
    }
}
