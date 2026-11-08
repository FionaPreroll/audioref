//! Parametric EQs: the filter bands of a correction, e.g. of one of OPRA's presets, with an overall
//! gain.
//!
//! A [`ParametricEq`] only holds values in the ranges below, which the filter design can handle at
//! every sample rate AudioRef supports: whatever a file says, a value outside them never reaches
//! the audio thread.
//!
//! ```
//! use audioref_core::eq::{Band, ParametricEq};
//!
//! let eq = ParametricEq::new(-4.4, vec![Band::LowShelf { frequency: 105.0, gain_db: 4.0, q: 0.7 }]);
//! assert!(eq.is_ok());
//! let eq = ParametricEq::new(0.0, vec![Band::Peak { frequency: 1000.0, gain_db: f64::NAN, q: 1.0 }]);
//! assert!(eq.is_err());
//! ```

use std::ops::RangeInclusive;

/// The most bands an EQ keeps. OPRA sorts the bands of a preset by priority, and asks software
/// with fewer bands to leave out the last ones; its presets have up to 20.
pub const MAX_BANDS: usize = 32;

/// The frequencies of bands, in Hz. Bands above half the sample rate are left out when the
/// filters are designed.
pub const FREQUENCY_HZ: RangeInclusive<f64> = 1.0..=100_000.0;

/// The gains of bands, in dB.
pub const BAND_GAIN_DB: RangeInclusive<f64> = -30.0..=30.0;

/// The overall gain of an EQ, in dB.
pub const GAIN_DB: RangeInclusive<f64> = -60.0..=30.0;

/// The Q of bands. OPRA's schema asks for at least 0.1.
pub const Q: RangeInclusive<f64> = 0.1..=100.0;

/// The slope of a low or high pass, in dB per octave: the order of the filter times 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slope {
    /// 6 dB per octave, first order.
    Db6,
    /// 12 dB per octave, second order.
    Db12,
    /// 18 dB per octave, third order.
    Db18,
    /// 24 dB per octave, fourth order.
    Db24,
    /// 30 dB per octave, fifth order.
    Db30,
    /// 36 dB per octave, sixth order.
    Db36,
}

impl Slope {
    /// All slopes, from the flattest.
    pub const ALL: [Self; 6] = [
        Self::Db6,
        Self::Db12,
        Self::Db18,
        Self::Db24,
        Self::Db30,
        Self::Db36,
    ];

    /// The slope of so many dB per octave, if there is one: 6, 12, 18, 24, 30 or 36.
    #[must_use]
    pub fn from_db_per_octave(db: f64) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|slope| f64::from(slope.db_per_octave()).to_bits() == db.to_bits())
    }

    /// The slope in dB per octave.
    #[must_use]
    pub fn db_per_octave(self) -> u8 {
        self.order() * 6
    }

    /// The order of the filter, from 1 to 6.
    #[must_use]
    pub fn order(self) -> u8 {
        match self {
            Self::Db6 => 1,
            Self::Db12 => 2,
            Self::Db18 => 3,
            Self::Db24 => 4,
            Self::Db30 => 5,
            Self::Db36 => 6,
        }
    }
}

/// A band of a parametric EQ, with its frequency in Hz and its gain in dB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Band {
    /// A peak or a dip around `frequency`.
    Peak {
        /// The center, in Hz.
        frequency: f64,
        /// The gain at the center, in dB.
        gain_db: f64,
        /// The width: the higher, the narrower.
        q: f64,
    },
    /// A shelf that changes the frequencies below `frequency`.
    LowShelf {
        /// Where the shelf has half its gain, in Hz.
        frequency: f64,
        /// The gain of the frequencies below, in dB.
        gain_db: f64,
        /// The steepness of the transition.
        q: f64,
    },
    /// A shelf that changes the frequencies above `frequency`.
    HighShelf {
        /// Where the shelf has half its gain, in Hz.
        frequency: f64,
        /// The gain of the frequencies above, in dB.
        gain_db: f64,
        /// The steepness of the transition.
        q: f64,
    },
    /// Takes away the frequencies above `frequency`.
    LowPass {
        /// The cutoff, in Hz.
        frequency: f64,
        /// How steeply the frequencies above fall.
        slope: Slope,
    },
    /// Takes away the frequencies below `frequency`.
    HighPass {
        /// The cutoff, in Hz.
        frequency: f64,
        /// How steeply the frequencies below fall.
        slope: Slope,
    },
    /// Keeps only the frequencies around `frequency`.
    BandPass {
        /// The center, in Hz.
        frequency: f64,
        /// The width: the higher, the narrower.
        q: f64,
    },
    /// Takes away the frequencies around `frequency`.
    BandStop {
        /// The center, in Hz.
        frequency: f64,
        /// The width: the higher, the narrower.
        q: f64,
    },
}

impl Band {
    /// The frequency of the band, in Hz.
    #[must_use]
    pub fn frequency(&self) -> f64 {
        match *self {
            Self::Peak { frequency, .. }
            | Self::LowShelf { frequency, .. }
            | Self::HighShelf { frequency, .. }
            | Self::LowPass { frequency, .. }
            | Self::HighPass { frequency, .. }
            | Self::BandPass { frequency, .. }
            | Self::BandStop { frequency, .. } => frequency,
        }
    }

    /// Whether the frequency, the gain and the Q of the band are in their ranges. They are never
    /// NaN or infinite then.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let gain_and_q = match *self {
            Self::Peak { gain_db, q, .. }
            | Self::LowShelf { gain_db, q, .. }
            | Self::HighShelf { gain_db, q, .. } => {
                BAND_GAIN_DB.contains(&gain_db) && Q.contains(&q)
            }
            Self::LowPass { .. } | Self::HighPass { .. } => true,
            Self::BandPass { q, .. } | Self::BandStop { q, .. } => Q.contains(&q),
        };
        gain_and_q && FREQUENCY_HZ.contains(&self.frequency())
    }
}

/// Why an EQ was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqError {
    /// The overall gain is outside [`GAIN_DB`], or not a number.
    Gain,
    /// A value of the band with this index, counted from 0, is outside its range.
    Band(usize),
}

/// A parametric EQ: an overall gain and up to [`MAX_BANDS`] bands, all in their ranges.
#[derive(Debug, Clone, PartialEq)]
pub struct ParametricEq {
    gain_db: f64,
    bands: Vec<Band>,
}

impl ParametricEq {
    /// The EQ with this overall gain, in dB, and the first [`MAX_BANDS`] of these bands.
    ///
    /// # Errors
    ///
    /// If the gain or a value of one of the bands that it keeps is outside its range.
    pub fn new(gain_db: f64, mut bands: Vec<Band>) -> Result<Self, EqError> {
        if !GAIN_DB.contains(&gain_db) {
            return Err(EqError::Gain);
        }
        bands.truncate(MAX_BANDS);
        if let Some(index) = bands.iter().position(|band| !band.is_valid()) {
            return Err(EqError::Band(index));
        }
        Ok(Self { gain_db, bands })
    }

    /// The overall gain, in dB.
    #[must_use]
    pub fn gain_db(&self) -> f64 {
        self.gain_db
    }

    /// The bands, in the order of the preset.
    #[must_use]
    pub fn bands(&self) -> &[Band] {
        &self.bands
    }
}

#[cfg(test)]
mod tests {
    use super::{Band, EqError, MAX_BANDS, ParametricEq, Slope};

    fn peak(frequency: f64, gain_db: f64, q: f64) -> Band {
        Band::Peak {
            frequency,
            gain_db,
            q,
        }
    }

    #[test]
    fn slopes_go_with_their_db_per_octave() {
        for (slope, order) in Slope::ALL.into_iter().zip(1..) {
            assert_eq!(slope.order(), order);
            assert_eq!(slope.db_per_octave(), order * 6);
            assert_eq!(Slope::from_db_per_octave(f64::from(order * 6)), Some(slope));
        }
        for db in [0.0, 3.0, 12.5, 42.0, -12.0, f64::NAN, f64::INFINITY] {
            assert_eq!(Slope::from_db_per_octave(db), None, "{db}");
        }
    }

    #[test]
    fn every_kind_of_band_has_a_frequency() {
        let bands = [
            peak(100.0, 1.0, 1.0),
            Band::LowShelf {
                frequency: 105.0,
                gain_db: 6.0,
                q: 0.7,
            },
            Band::HighShelf {
                frequency: 10_000.0,
                gain_db: -2.0,
                q: 0.7,
            },
            Band::LowPass {
                frequency: 12_000.0,
                slope: Slope::Db12,
            },
            Band::HighPass {
                frequency: 20.0,
                slope: Slope::Db24,
            },
            Band::BandPass {
                frequency: 1000.0,
                q: 2.0,
            },
            Band::BandStop {
                frequency: 50.0,
                q: 10.0,
            },
        ];
        let frequencies = [100.0, 105.0, 10_000.0, 12_000.0, 20.0, 1000.0, 50.0];
        for (band, frequency) in bands.iter().zip(frequencies) {
            assert!(band.is_valid(), "{band:?}");
            assert_eq!(band.frequency().to_bits(), f64::to_bits(frequency));
        }
    }

    #[test]
    fn values_outside_their_ranges_are_invalid() {
        let invalid = [
            peak(0.5, 0.0, 1.0),
            peak(100_001.0, 0.0, 1.0),
            peak(f64::NAN, 0.0, 1.0),
            peak(100.0, 30.5, 1.0),
            peak(100.0, f64::NEG_INFINITY, 1.0),
            peak(100.0, 0.0, 0.05),
            peak(100.0, 0.0, f64::NAN),
            Band::LowShelf {
                frequency: 100.0,
                gain_db: 0.0,
                q: 101.0,
            },
            Band::HighShelf {
                frequency: 100.0,
                gain_db: -31.0,
                q: 1.0,
            },
            Band::LowPass {
                frequency: f64::INFINITY,
                slope: Slope::Db6,
            },
            Band::HighPass {
                frequency: 0.0,
                slope: Slope::Db36,
            },
            Band::BandPass {
                frequency: 100.0,
                q: 0.0,
            },
            Band::BandStop {
                frequency: -5.0,
                q: 1.0,
            },
        ];
        for band in invalid {
            assert!(!band.is_valid(), "{band:?}");
        }
        assert!(peak(1.0, -30.0, 0.1).is_valid());
        assert!(peak(100_000.0, 30.0, 100.0).is_valid());
    }

    #[test]
    fn an_eq_keeps_valid_bands_in_their_order() {
        let bands = vec![peak(1000.0, -3.0, 1.4), peak(60.0, 2.0, 0.7)];
        let eq = ParametricEq::new(-4.0, bands.clone()).expect("accepted");
        assert_eq!(eq.gain_db().to_bits(), f64::to_bits(-4.0));
        assert_eq!(eq.bands(), &bands[..]);
        assert!(ParametricEq::new(0.0, Vec::new()).is_ok());
    }

    #[test]
    fn an_eq_refuses_a_bad_gain_or_band() {
        for gain_db in [f64::NAN, 30.5, -61.0, f64::INFINITY] {
            assert_eq!(ParametricEq::new(gain_db, Vec::new()), Err(EqError::Gain));
        }
        let bands = vec![peak(1000.0, -3.0, 1.4), peak(60.0, 2.0, 0.0)];
        assert_eq!(ParametricEq::new(0.0, bands), Err(EqError::Band(1)));
    }

    #[test]
    fn an_eq_keeps_only_the_first_bands() {
        let mut bands: Vec<Band> = (0..MAX_BANDS).map(|_| peak(100.0, 1.0, 1.0)).collect();
        // The bands it leaves out aren't checked.
        bands.push(peak(f64::NAN, 1.0, 1.0));
        let eq = ParametricEq::new(0.0, bands).expect("accepted");
        assert_eq!(eq.bands().len(), MAX_BANDS);
    }
}
