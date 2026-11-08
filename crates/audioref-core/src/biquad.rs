//! Biquad filters for the bands of a [`ParametricEq`], designed after Martin Vicanek's
//! "Matched Second Order Digital Filters" (2016).
//!
//! The usual formulas, e.g. those of Robert Bristow-Johnson's cookbook, warp the frequencies with
//! the bilinear transform: at 44.1 kHz a peak at 10 kHz comes out narrower than it should, and
//! the response bends down toward half the sample rate. The matched design keeps the poles of the
//! analog filter, by impulse invariance, and chooses the zeros so the magnitude is the analog one
//! at 0 Hz, at the band's frequency and at half the sample rate. The result follows the analog
//! filter closely up to half the sample rate, with zero latency: peaks and shelves up to 10 kHz
//! within 0.75 dB at 44.1 kHz and above, most within a tenth of that; passes and stops within 2 dB
//! where they let through more than -12 dB. High passes have both zeros at 0 Hz, band stops on the
//! unit circle at their frequency, first-order passes match the analog one at 0 Hz and 3 dB down.
//!
//! ```
//! use audioref_core::biquad;
//! use audioref_core::eq::Band;
//!
//! let peak = Band::Peak { frequency: 10_000.0, gain_db: 6.0, q: 2.0 };
//! let sections = biquad::design(&peak, 44_100.0);
//! let db: f64 = sections.iter().map(|section| section.response_db(10_000.0, 44_100.0)).sum();
//! assert!((db - 6.0).abs() < 0.01);
//! ```
//!
//! [`ParametricEq`]: crate::eq::ParametricEq

use std::f64::consts::PI;

use crate::eq::{Band, Slope};

/// The coefficients of a biquad, normalized to `a0 = 1`:
/// `y[n] = b0 x[n] + b1 x[n-1] + b2 x[n-2] - a1 y[n-1] - a2 y[n-2]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coefficients {
    /// The gain of the input.
    pub b0: f64,
    /// The gain of the input one sample ago.
    pub b1: f64,
    /// The gain of the input two samples ago.
    pub b2: f64,
    /// The feedback of the output one sample ago.
    pub a1: f64,
    /// The feedback of the output two samples ago.
    pub a2: f64,
}

impl Coefficients {
    /// The biquad that passes the signal through unchanged.
    pub const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// The magnitude of the response at a frequency, both in Hz, as a factor.
    #[must_use]
    pub fn response(&self, frequency: f64, sample_rate: f64) -> f64 {
        let phi = Phi::at(2.0 * PI * frequency / sample_rate);
        let numerator = phi.weigh(Squares::of(self.b0, self.b1, self.b2));
        let denominator = phi.weigh(Squares::of(1.0, self.a1, self.a2));
        (numerator.max(0.0) / denominator).sqrt()
    }

    /// The magnitude of the response at a frequency, both in Hz, in dB.
    #[must_use]
    pub fn response_db(&self, frequency: f64, sample_rate: f64) -> f64 {
        20.0 * self.response(frequency, sample_rate).log10()
    }

    /// Whether all coefficients are finite and the poles inside the unit circle, so the filter
    /// neither blows up nor rings forever.
    #[must_use]
    pub fn is_stable(&self) -> bool {
        [self.b0, self.b1, self.b2, self.a1, self.a2]
            .iter()
            .all(|value| value.is_finite())
            && self.a2.abs() < 1.0
            && self.a1.abs() < 1.0 + self.a2
    }
}

/// The squared magnitude of a polynomial `c0 + c1 z⁻¹ + c2 z⁻²` on the unit circle, as Vicanek
/// writes it: `S0 φ0 + S1 φ1 + S2 φ2`.
#[derive(Debug, Clone, Copy)]
struct Squares {
    s0: f64,
    s1: f64,
    s2: f64,
}

impl Squares {
    fn of(c0: f64, c1: f64, c2: f64) -> Self {
        Self {
            s0: (c0 + c1 + c2).powi(2),
            s1: (c0 - c1 + c2).powi(2),
            s2: -4.0 * c0 * c2,
        }
    }
}

/// Vicanek's weights at an angular frequency ω, in radians per sample: `φ1 = sin²(ω/2)`,
/// `φ0 = 1 - φ1` and `φ2 = 4 φ0 φ1`.
#[derive(Debug, Clone, Copy)]
struct Phi {
    phi0: f64,
    phi1: f64,
    phi2: f64,
}

impl Phi {
    fn at(omega: f64) -> Self {
        let phi1 = (omega / 2.0).sin().powi(2);
        let phi0 = 1.0 - phi1;
        Self {
            phi0,
            phi1,
            phi2: 4.0 * phi0 * phi1,
        }
    }

    fn weigh(self, squares: Squares) -> f64 {
        squares.s0 * self.phi0 + squares.s1 * self.phi1 + squares.s2 * self.phi2
    }
}

/// The biquads of a band at a sample rate, in Hz: one for peaks, shelves, band passes and band
/// stops, one to three for low and high passes, by their slope. None for a band at or above half
/// the sample rate, which leaves it out, or one that gives no stable filter, e.g. with a gain that
/// isn't a number, which a [`ParametricEq`](crate::eq::ParametricEq) never holds.
#[must_use]
pub fn design(band: &Band, sample_rate: f64) -> Vec<Coefficients> {
    let frequency = band.frequency();
    if !(sample_rate.is_finite() && frequency > 0.0 && frequency < sample_rate / 2.0) {
        return Vec::new();
    }
    let omega = 2.0 * PI * frequency / sample_rate;
    let sections = match *band {
        Band::Peak { gain_db, q, .. } => {
            let a = 10.0_f64.powf(gain_db / 40.0);
            vec![matched(omega, a * q, omega, Zeros::Matched, &|w| {
                let x = w * w;
                let num = (1.0 - x).powi(2) + (a * w / q).powi(2);
                let den = (1.0 - x).powi(2) + (w / (a * q)).powi(2);
                num / den
            })]
        }
        Band::LowShelf { gain_db, q, .. } => {
            let a = 10.0_f64.powf(gain_db / 40.0);
            vec![matched(omega / a.sqrt(), q, omega, Zeros::Matched, &|w| {
                let x = w * w;
                let num = a * a * ((a - x).powi(2) + a * x / (q * q));
                let den = (1.0 - a * x).powi(2) + a * x / (q * q);
                num / den
            })]
        }
        Band::HighShelf { gain_db, q, .. } => {
            let a = 10.0_f64.powf(gain_db / 40.0);
            vec![matched(omega * a.sqrt(), q, omega, Zeros::Matched, &|w| {
                let x = w * w;
                let num = a * a * ((1.0 - a * x).powi(2) + a * x / (q * q));
                let den = (a - x).powi(2) + a * x / (q * q);
                num / den
            })]
        }
        Band::BandPass { q, .. } => vec![matched(omega, q, omega, Zeros::Matched, &|w| {
            let bandwidth = (w / q).powi(2);
            bandwidth / ((1.0 - w * w).powi(2) + bandwidth)
        })],
        Band::BandStop { q, .. } => vec![matched(omega, q, omega, Zeros::AtBand, &|w| {
            let notch = (1.0 - w * w).powi(2);
            notch / (notch + (w / q).powi(2))
        })],
        Band::LowPass { slope, .. } => butterworth(omega, slope, false),
        Band::HighPass { slope, .. } => butterworth(omega, slope, true),
    };
    if sections.iter().all(Coefficients::is_stable) {
        sections
    } else {
        Vec::new()
    }
}

/// A Butterworth low or high pass of a slope at ω, in radians per sample: a biquad for each pair
/// of poles, and a first-order section for an odd order.
fn butterworth(omega: f64, slope: Slope, high: bool) -> Vec<Coefficients> {
    let order = slope.order();
    let mut sections: Vec<Coefficients> = (1..=order / 2)
        .map(|k| {
            // The poles of a pair lie at this angle from the negative real axis.
            let angle = PI * f64::from(2 * k - 1 + order % 2) / f64::from(2 * order);
            let q = 1.0 / (2.0 * angle.cos());
            let zeros = if high { Zeros::AtZero } else { Zeros::Matched };
            matched(omega, q, omega, zeros, &move |w| {
                let x = w * w;
                let den = (1.0 - x).powi(2) + x / (q * q);
                if high { x * x / den } else { 1.0 / den }
            })
        })
        .collect();
    if order % 2 == 1 {
        sections.push(first_order(omega, high));
    }
    sections
}

/// A first-order low or high pass at ω, in radians per sample: its pole by impulse invariance,
/// its zero so the magnitude is the analog one at 0 Hz and at ω, 3 dB down.
fn first_order(omega: f64, high: bool) -> Coefficients {
    let pole = (-omega).exp();
    let phi = Phi::at(omega);
    // The squared magnitude of 1 - pole z⁻¹ at ω, the sum of the b at 0 Hz, and the difference of
    // the b that brings the magnitude at ω to half its power.
    let at_omega = (1.0 - pole).powi(2) * phi.phi0 + (1.0 + pole).powi(2) * phi.phi1;
    let sum = if high { 0.0 } else { 1.0 - pole };
    let difference = ((at_omega / 2.0 - sum * sum * phi.phi0).max(0.0) / phi.phi1).sqrt();
    Coefficients {
        b0: f64::midpoint(sum, difference),
        b1: (sum - difference) / 2.0,
        b2: 0.0,
        a1: -pole,
        a2: 0.0,
    }
}

/// Where a matched biquad puts its zeros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zeros {
    /// Where the magnitude is the analog one at 0 Hz, at the band's frequency and at half the
    /// sample rate.
    Matched,
    /// Both at 0 Hz, as for a high pass, with the analog magnitude at half the sample rate.
    AtZero,
    /// On the unit circle at the band's frequency, as for a band stop, with the analog magnitude
    /// at 0 Hz.
    AtBand,
}

/// Vicanek's matched biquad: the poles of an analog biquad with its poles at `pole_omega`, in
/// radians per sample, and their Q, and the zeros that `zeros` says, for `magnitude`, the analog
/// squared magnitude as a function of the frequency divided by that of the band, which lies at
/// `omega`.
fn matched(
    pole_omega: f64,
    pole_q: f64,
    omega: f64,
    zeros: Zeros,
    magnitude: &dyn Fn(f64) -> f64,
) -> Coefficients {
    let zeta = 1.0 / (2.0 * pole_q);
    let decay = (-zeta * pole_omega).exp();
    let a1 = if zeta <= 1.0 {
        -2.0 * decay * ((1.0 - zeta * zeta).sqrt() * pole_omega).cos()
    } else {
        -2.0 * decay * ((zeta * zeta - 1.0).sqrt() * pole_omega).cosh()
    };
    let a2 = decay * decay;
    let poles = Squares::of(1.0, a1, a2);
    let (b0, b1, b2) = match zeros {
        Zeros::Matched => {
            let phi = Phi::at(omega);
            let b0_sq = poles.s0 * magnitude(0.0);
            let b1_sq = poles.s1 * magnitude(PI / omega);
            let b2_sq = (magnitude(1.0) * phi.weigh(poles) - b0_sq * phi.phi0 - b1_sq * phi.phi1)
                / phi.phi2;
            let (root0, root1) = (b0_sq.max(0.0).sqrt(), b1_sq.max(0.0).sqrt());
            let w = f64::midpoint(root0, root1);
            let b0 = f64::midpoint(w, (w * w + b2_sq).max(0.0).sqrt());
            (b0, (root0 - root1) / 2.0, -b2_sq / (4.0 * b0))
        }
        Zeros::AtZero => {
            // (1 - z⁻¹)² is 4 at half the sample rate.
            let b0 = (poles.s1 * magnitude(PI / omega)).sqrt() / 4.0;
            (b0, -2.0 * b0, b0)
        }
        Zeros::AtBand => {
            // 1 - 2 cos ω z⁻¹ + z⁻² is 2 - 2 cos ω at 0 Hz.
            let b0 = (poles.s0 * magnitude(0.0)).sqrt() / (2.0 - 2.0 * omega.cos());
            (b0, -2.0 * omega.cos() * b0, b0)
        }
    };
    Coefficients { b0, b1, b2, a1, a2 }
}

#[cfg(test)]
mod tests {
    use super::{Coefficients, design};
    use crate::eq::{Band, Slope};

    /// The analog responses of the bands, in dB, at a frequency, for comparison.
    fn analog_db(band: &Band, frequency: f64) -> f64 {
        let w = frequency / band.frequency();
        let x = w * w;
        let squared = match *band {
            Band::Peak { gain_db, q, .. } => {
                let a = 10.0_f64.powf(gain_db / 40.0);
                ((1.0 - x).powi(2) + (a * w / q).powi(2))
                    / ((1.0 - x).powi(2) + (w / (a * q)).powi(2))
            }
            Band::LowShelf { gain_db, q, .. } => {
                let a = 10.0_f64.powf(gain_db / 40.0);
                a * a * ((a - x).powi(2) + a * x / (q * q))
                    / ((1.0 - a * x).powi(2) + a * x / (q * q))
            }
            Band::HighShelf { gain_db, q, .. } => {
                let a = 10.0_f64.powf(gain_db / 40.0);
                a * a * ((1.0 - a * x).powi(2) + a * x / (q * q))
                    / ((a - x).powi(2) + a * x / (q * q))
            }
            Band::BandPass { q, .. } => (w / q).powi(2) / ((1.0 - x).powi(2) + (w / q).powi(2)),
            Band::BandStop { q, .. } => (1.0 - x).powi(2) / ((1.0 - x).powi(2) + (w / q).powi(2)),
            Band::LowPass { slope, .. } => 1.0 / (1.0 + x.powi(i32::from(slope.order()))),
            Band::HighPass { slope, .. } => {
                let n = x.powi(i32::from(slope.order()));
                n / (1.0 + n)
            }
        };
        10.0 * squared.log10()
    }

    fn digital_db(sections: &[Coefficients], frequency: f64, sample_rate: f64) -> f64 {
        sections
            .iter()
            .map(|section| section.response_db(frequency, sample_rate))
            .sum()
    }

    /// The frequencies from 20 Hz up to just below half the sample rate, a third of an octave
    /// apart.
    fn frequencies(sample_rate: f64) -> impl Iterator<Item = f64> {
        (0..)
            .map(|i| 20.0 * 2.0_f64.powf(f64::from(i) / 3.0))
            .take_while(move |frequency| *frequency < 0.49 * sample_rate)
    }

    /// Peaks and shelves as OPRA's presets have them, up to 10 kHz.
    fn peaks_and_shelves() -> Vec<Band> {
        let mut bands = Vec::new();
        for frequency in [30.0, 105.0, 1000.0, 6000.0, 10_000.0] {
            for (gain_db, q) in [(6.0, 0.7), (-9.0, 2.5), (12.0, 0.3), (-3.0, 6.0)] {
                bands.push(Band::Peak {
                    frequency,
                    gain_db,
                    q,
                });
                bands.push(Band::LowShelf {
                    frequency,
                    gain_db,
                    q: q.min(1.0),
                });
                bands.push(Band::HighShelf {
                    frequency,
                    gain_db,
                    q: q.min(1.0),
                });
            }
        }
        bands
    }

    /// Passes and stops up to 10 kHz.
    fn passes_and_stops() -> Vec<Band> {
        let mut bands = Vec::new();
        for frequency in [30.0, 105.0, 1000.0, 6000.0, 10_000.0] {
            for q in [0.5, 2.0] {
                bands.push(Band::BandPass { frequency, q });
                bands.push(Band::BandStop { frequency, q });
            }
            for slope in Slope::ALL {
                bands.push(Band::LowPass { frequency, slope });
                bands.push(Band::HighPass { frequency, slope });
            }
        }
        bands
    }

    /// The largest difference to the analog response, in dB, at the frequencies up to
    /// `up_to` times the sample rate where the analog response is above `floor_db`.
    fn largest_error(band: &Band, sample_rate: f64, up_to: f64, floor_db: f64) -> f64 {
        let sections = design(band, sample_rate);
        assert!(!sections.is_empty(), "{band:?} at {sample_rate}");
        frequencies(sample_rate)
            .take_while(|frequency| *frequency <= up_to * sample_rate)
            .filter(|frequency| analog_db(band, *frequency) > floor_db)
            .map(|frequency| {
                (analog_db(band, frequency) - digital_db(&sections, frequency, sample_rate)).abs()
            })
            .fold(0.0, f64::max)
    }

    const SAMPLE_RATES: [f64; 4] = [44_100.0, 48_000.0, 96_000.0, 192_000.0];

    #[test]
    fn peaks_and_shelves_follow_the_analog_filters_up_to_half_the_sample_rate() {
        for sample_rate in SAMPLE_RATES {
            for band in peaks_and_shelves() {
                let error = largest_error(&band, sample_rate, 0.5, -100.0);
                // The widest boost, by 12 dB with a Q of 0.3, differs most, by 0.7 dB at 16 kHz.
                assert!(error < 0.75, "{band:?} at {sample_rate} Hz: {error:.2} dB");
            }
        }
    }

    #[test]
    fn passes_and_stops_follow_the_analog_filters_in_their_pass_band() {
        for sample_rate in SAMPLE_RATES {
            for band in passes_and_stops() {
                let error = largest_error(&band, sample_rate, 0.5, -12.0);
                assert!(error < 2.0, "{band:?} at {sample_rate} Hz: {error:.2} dB");
            }
        }
    }

    #[test]
    fn matches_at_the_band_frequency_and_half_the_sample_rate() {
        let sample_rate = 44_100.0;
        let band = Band::Peak {
            frequency: 12_000.0,
            gain_db: 9.0,
            q: 1.0,
        };
        let sections = design(&band, sample_rate);
        for frequency in [12_000.0, sample_rate / 2.0] {
            let error = digital_db(&sections, frequency, sample_rate) - analog_db(&band, frequency);
            assert!(error.abs() < 1e-6, "{frequency}: {error}");
        }
    }

    #[test]
    fn bands_at_or_above_half_the_sample_rate_are_left_out() {
        for frequency in [22_050.0, 30_000.0] {
            assert!(
                design(
                    &Band::Peak {
                        frequency,
                        gain_db: 3.0,
                        q: 1.0
                    },
                    44_100.0
                )
                .is_empty()
            );
        }
        let band = Band::LowPass {
            frequency: 1000.0,
            slope: Slope::Db12,
        };
        assert!(design(&band, f64::NAN).is_empty());
        let band = Band::Peak {
            frequency: 1000.0,
            gain_db: f64::NAN,
            q: 1.0,
        };
        assert!(design(&band, 48_000.0).is_empty());
        assert!(design(&band, f64::INFINITY).is_empty());
    }

    #[test]
    fn slopes_give_their_number_of_sections() {
        let counts: Vec<usize> = Slope::ALL
            .into_iter()
            .map(|slope| {
                design(
                    &Band::HighPass {
                        frequency: 100.0,
                        slope,
                    },
                    48_000.0,
                )
                .len()
            })
            .collect();
        assert_eq!(counts, [1, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn extreme_bands_stay_stable() {
        let mut extreme = Vec::new();
        for frequency in [1.0, 21_900.0, 95_000.0] {
            for (gain_db, q) in [(30.0, 0.1), (-30.0, 100.0), (30.0, 100.0), (-30.0, 0.1)] {
                extreme.push(Band::Peak {
                    frequency,
                    gain_db,
                    q,
                });
                extreme.push(Band::LowShelf {
                    frequency,
                    gain_db,
                    q,
                });
                extreme.push(Band::HighShelf {
                    frequency,
                    gain_db,
                    q,
                });
            }
            extreme.push(Band::BandStop {
                frequency,
                q: 100.0,
            });
            extreme.push(Band::BandPass { frequency, q: 0.1 });
            extreme.push(Band::HighPass {
                frequency,
                slope: Slope::Db36,
            });
        }
        for sample_rate in [44_100.0, 192_000.0] {
            for band in &extreme {
                for section in design(band, sample_rate) {
                    assert!(
                        section.is_stable(),
                        "{band:?} at {sample_rate}: {section:?}"
                    );
                    assert!(section.response(1000.0, sample_rate).is_finite());
                }
            }
        }
    }

    #[test]
    fn stability() {
        assert!(Coefficients::IDENTITY.is_stable());
        let unstable = [
            Coefficients {
                a2: 1.0,
                ..Coefficients::IDENTITY
            },
            Coefficients {
                a1: -2.1,
                a2: 0.5,
                ..Coefficients::IDENTITY
            },
            Coefficients {
                b1: f64::NAN,
                ..Coefficients::IDENTITY
            },
        ];
        for coefficients in unstable {
            assert!(!coefficients.is_stable(), "{coefficients:?}");
        }
    }

    #[test]
    fn the_identity_passes_every_frequency() {
        for frequency in [0.0, 20.0, 1000.0, 22_050.0] {
            assert!(
                Coefficients::IDENTITY
                    .response_db(frequency, 44_100.0)
                    .abs()
                    < 1e-12
            );
        }
        // A biquad's response at 0 Hz is the sum of its b over the sum of its a.
        let lowpass = Coefficients {
            b0: 0.25,
            b1: 0.5,
            b2: 0.25,
            a1: -0.5,
            a2: 0.0,
        };
        assert!((lowpass.response(0.0, 48_000.0) - 2.0).abs() < 1e-12);
        assert!(lowpass.response(24_000.0, 48_000.0) < 1e-12);
    }
}
