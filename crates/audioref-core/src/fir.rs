//! A [`Correction`] as a minimum-phase FIR filter: all its delay as early as the magnitude allows,
//! so it adds no latency and rings as little as possible before a transient.
//!
//! The filter comes from the real cepstrum: the logarithm of the magnitude, transformed back to
//! the time domain, folded onto its causal half and transformed forward again, gives the
//! logarithm of the minimum-phase spectrum; its exponential, transformed back, is the impulse
//! response. The FFTs are 8 times as long as the filter, so the cepstrum barely aliases.
//!
//! The overall gain is lowered by [`Correction::headroom_db`], so the filter boosts nothing.
//! [`minimum_phase`] allocates and takes a few milliseconds, so it runs in the background.
//!
//! ```
//! use audioref_core::design::Correction;
//! use audioref_core::fir;
//!
//! let flat = Correction::from_levels(vec![0.0; 695]).ok_or("not a correction")?;
//! let taps = fir::minimum_phase(&flat, 48_000.0).ok_or("no filter")?;
//! assert_eq!(taps.len(), 4096);
//! // Flat: an impulse at the very start.
//! assert!((taps[0] - 1.0).abs() < 1e-3);
//! assert!(taps[1..].iter().all(|tap| tap.abs() < 1e-3));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::f64::consts::{LN_10, PI};

use realfft::RealFftPlanner;
use realfft::num_complex::Complex;

use crate::design::Correction;

/// How many times as long as the filter the FFTs are.
const OVERSAMPLING: usize = 8;

/// The share of the filter, at its end, that fades out, so it ends without a step.
const FADE_SHARE: usize = 8;

/// The length of the filter at a sample rate in Hz: 4096 taps up to 48 kHz, 8192 up to 96 kHz,
/// 16384 above, so it reaches equally low frequencies at every sample rate.
#[must_use]
pub fn taps(sample_rate: f64) -> usize {
    if sample_rate <= 50_000.0 {
        4096
    } else if sample_rate <= 100_000.0 {
        8192
    } else {
        16_384
    }
}

/// The minimum-phase filter of a correction at a sample rate in Hz, [`taps`] long, with the
/// overall gain lowered by its headroom. `None` only if an FFT fails, which it doesn't for these
/// lengths.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "lengths of at most 2^17 are exact in f64, and the taps are small"
)]
pub fn minimum_phase(correction: &Correction, sample_rate: f64) -> Option<Vec<f32>> {
    let length = taps(sample_rate);
    let size = length * OVERSAMPLING;
    let mut planner = RealFftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(size);
    let inverse = planner.plan_fft_inverse(size);
    let gain_db = -correction.headroom_db();

    // The logarithm of the magnitude, in nepers, at each bin.
    let mut spectrum = inverse.make_input_vec();
    for (bin, value) in spectrum.iter_mut().enumerate() {
        let frequency = bin as f64 * sample_rate / size as f64;
        let db = correction.response_db(frequency) + gain_db;
        *value = Complex::new(db * LN_10 / 20.0, 0.0);
    }
    let mut cepstrum = inverse.make_output_vec();
    inverse.process(&mut spectrum, &mut cepstrum).ok()?;

    // Folded onto its causal half, and scaled for the unnormalized FFTs.
    let half = size / 2;
    for (index, value) in cepstrum.iter_mut().enumerate() {
        let weight = match index {
            0 => 1.0,
            _ if index < half => 2.0,
            _ if index == half => 1.0,
            _ => 0.0,
        };
        *value *= weight / size as f64;
    }
    let mut logarithm = forward.make_output_vec();
    forward.process(&mut cepstrum, &mut logarithm).ok()?;

    // The minimum-phase spectrum, and its impulse response.
    // The bins at 0 Hz and at half the sample rate are real, as the inverse FFT requires.
    for (bin, value) in logarithm.iter_mut().enumerate() {
        *value = Complex::from_polar(value.re.exp(), value.im);
        if bin == 0 || bin == half {
            value.im = 0.0;
        }
    }
    let mut impulse = inverse.make_output_vec();
    inverse.process(&mut logarithm, &mut impulse).ok()?;

    let fade_start = length - length / FADE_SHARE;
    let taps = impulse
        .iter()
        .take(length)
        .enumerate()
        .map(|(index, value)| {
            let fade = if index < fade_start {
                1.0
            } else {
                let share = (index - fade_start) as f64 / (length - fade_start) as f64;
                0.5 + 0.5 * (PI * share).cos()
            };
            (value / size as f64 * fade) as f32
        })
        .collect();
    Some(taps)
}

#[cfg(test)]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "bins of FFTs of at most 2^16 points"
)]
mod tests {
    use super::*;
    use crate::autoeq::Curve;
    use crate::cascade::grid;
    use crate::design::Settings;

    /// The magnitude of a filter at the frequencies of the grid below `top` Hz, in dB.
    fn response_db(taps: &[f32], sample_rate: f64, top: f64) -> Vec<(f64, f64)> {
        let size = taps.len() * 4;
        let mut planner = RealFftPlanner::<f64>::new();
        let forward = planner.plan_fft_forward(size);
        let mut input: Vec<f64> = taps.iter().map(|&tap| f64::from(tap)).collect();
        input.resize(size, 0.0);
        let mut spectrum = forward.make_output_vec();
        forward.process(&mut input, &mut spectrum).unwrap();
        grid()
            .filter(|&f| f < top)
            .map(|f| {
                // Between two bins, the nearer one.
                let bin = (f * size as f64 / sample_rate).round() as usize;
                (f, 20.0 * spectrum[bin].norm().log10())
            })
            .collect()
    }

    fn correction() -> Correction {
        // Like real headphones: a bass that rolls off, a dip at 3 kHz, a peak at 9 kHz.
        let measurement = Curve::from_points(
            grid()
                .map(|f: f64| {
                    let octaves = |center: f64| (f / center).log2();
                    let bass = -6.0 * (1.0 - (octaves(60.0).max(0.0) / 2.0).min(1.0));
                    let dip = -5.0 * (-octaves(3000.0).powi(2) * 8.0).exp();
                    let peak = 6.0 * (-octaves(9000.0).powi(2) * 8.0).exp();
                    (f, bass + dip + peak)
                })
                .collect(),
        )
        .unwrap();
        let target = Curve::from_points(vec![(20.0, 0.0), (20_000.0, 0.0)]).unwrap();
        Correction::design(&measurement, &target, Settings::default())
    }

    #[test]
    fn lengths() {
        assert_eq!(taps(44_100.0), 4096);
        assert_eq!(taps(48_000.0), 4096);
        assert_eq!(taps(88_200.0), 8192);
        assert_eq!(taps(96_000.0), 8192);
        assert_eq!(taps(176_400.0), 16_384);
        assert_eq!(taps(192_000.0), 16_384);
    }

    #[test]
    fn follows_the_correction_at_every_sample_rate() {
        let correction = correction();
        let headroom = correction.headroom_db();
        assert!(headroom > 5.0);
        for sample_rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 176_400.0, 192_000.0] {
            let taps = minimum_phase(&correction, sample_rate).unwrap();
            assert_eq!(taps.len(), super::taps(sample_rate));
            let mut worst = (0.0, 0.0_f64);
            for (f, db) in response_db(&taps, sample_rate, 20_000.0) {
                let error = (db - (correction.response_db(f) - headroom)).abs();
                if f >= 30.0 && error > worst.1 {
                    worst = (f, error);
                }
                assert!(db < 0.1, "{db} dB at {f} Hz, {sample_rate} Hz");
            }
            assert!(
                worst.1 < 0.5,
                "{} dB off at {} Hz, {sample_rate} Hz",
                worst.1,
                worst.0
            );
        }
    }

    #[test]
    fn is_minimum_phase() {
        let taps = minimum_phase(&correction(), 48_000.0).unwrap();
        let energy = |taps: &[f32]| taps.iter().map(|tap| f64::from(*tap).powi(2)).sum::<f64>();
        // Almost all of its energy comes in the first 2 ms, and the start is no silence.
        assert!(energy(&taps[..96]) > 0.98 * energy(&taps));
        assert!(taps[0].abs() > 0.1);
    }
}
