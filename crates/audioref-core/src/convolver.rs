//! The convolution of a signal with a long FIR filter, without latency, for each channel.
//!
//! The first [`BLOCK`] taps run in the time domain, sample by sample. The rest is split into
//! partitions of [`BLOCK`] taps, convolved block by block with FFTs of twice that length
//! (uniformly partitioned overlap-save): their contribution to a block depends only on the blocks
//! before it, so it is ready when the block starts, and the output has no delay.
//!
//! [`Convolver::new`] allocates, so it runs in the background; [`Convolver::process`] doesn't: the
//! FFTs work in buffers made beforehand.
//!
//! ```
//! use audioref_core::convolver::Convolver;
//!
//! // A delay of 300 samples, at half the level.
//! let mut taps = vec![0.0_f32; 301];
//! taps[300] = 0.5;
//! let mut convolver = Convolver::new(&taps);
//! let output: Vec<f64> = (0..400)
//!     .map(|n| convolver.process(0, if n == 0 { 1.0 } else { 0.0 }))
//!     .collect();
//! assert!((output[300] - 0.5).abs() < 1e-6);
//! assert!(output.iter().enumerate().all(|(n, y)| n == 300 || y.abs() < 1e-6));
//! ```

use std::fmt;
use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

use crate::cascade::CHANNELS;

/// The length of the part in the time domain, of the partitions and of the blocks, in samples.
pub const BLOCK: usize = 128;

/// The state of one channel.
#[derive(Debug, Clone, PartialEq)]
struct Channel {
    /// The last [`BLOCK`] samples, twice: the samples from `position + 1` on are in order.
    recent: Vec<f32>,
    /// The block before and the block that is being filled.
    input: Vec<f32>,
    /// How many samples of the current block came in.
    position: usize,
    /// The spectra of the last blocks, one for each partition, the newest at `newest`.
    spectra: Vec<Vec<Complex<f32>>>,
    newest: usize,
    /// What the partitions add to each sample of the current block.
    tail: Vec<f32>,
}

impl Channel {
    fn new(partitions: usize) -> Self {
        Self {
            recent: vec![0.0; 2 * BLOCK],
            input: vec![0.0; 2 * BLOCK],
            position: 0,
            spectra: vec![vec![Complex::default(); BLOCK + 1]; partitions],
            newest: 0,
            tail: vec![0.0; BLOCK],
        }
    }
}

/// The buffers the FFTs work in, shared by the channels.
#[derive(Debug, Clone)]
struct Work {
    time: Vec<f32>,
    spectrum: Vec<Complex<f32>>,
    sum: Vec<Complex<f32>>,
    scratch_forward: Vec<Complex<f32>>,
    scratch_inverse: Vec<Complex<f32>>,
}

/// A FIR filter with the state of each channel.
#[derive(Clone)]
pub struct Convolver {
    /// The first [`BLOCK`] taps, the last first.
    head: Vec<f32>,
    /// The spectra of the partitions after the head, scaled for the FFTs.
    partitions: Vec<Vec<Complex<f32>>>,
    forward: Arc<dyn RealToComplex<f32>>,
    inverse: Arc<dyn ComplexToReal<f32>>,
    channels: [Channel; CHANNELS],
    work: Work,
}

impl fmt::Debug for Convolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Convolver")
            .field("head", &self.head)
            .field("partitions", &self.partitions.len())
            .field("channels", &self.channels)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Convolver {
    /// The same taps and the same state. The plans of the FFTs depend only on [`BLOCK`], and the
    /// buffers they work in hold nothing from one block to the next.
    fn eq(&self, other: &Self) -> bool {
        self.head == other.head
            && self.partitions == other.partitions
            && self.channels == other.channels
    }
}

impl Convolver {
    /// A convolver for these taps, with silence in each channel.
    #[must_use]
    #[allow(clippy::cast_precision_loss, reason = "twice BLOCK is exact in f32")]
    pub fn new(taps: &[f32]) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let forward = planner.plan_fft_forward(2 * BLOCK);
        let inverse = planner.plan_fft_inverse(2 * BLOCK);
        let mut head: Vec<f32> = taps.iter().take(BLOCK).copied().collect();
        head.resize(BLOCK, 0.0);
        head.reverse();
        let scale = 1.0 / (2 * BLOCK) as f32;
        let mut scratch = forward.make_scratch_vec();
        let partitions: Vec<Vec<Complex<f32>>> = taps
            .get(BLOCK..)
            .unwrap_or_default()
            .chunks(BLOCK)
            .map(|chunk| {
                let mut time: Vec<f32> = chunk.iter().map(|tap| tap * scale).collect();
                time.resize(2 * BLOCK, 0.0);
                let mut spectrum = forward.make_output_vec();
                // The lengths are those of the plan, so this doesn't fail.
                drop(forward.process_with_scratch(&mut time, &mut spectrum, &mut scratch));
                spectrum
            })
            .collect();
        let work = Work {
            time: forward.make_input_vec(),
            spectrum: forward.make_output_vec(),
            sum: forward.make_output_vec(),
            scratch_forward: scratch,
            scratch_inverse: inverse.make_scratch_vec(),
        };
        Self {
            channels: std::array::from_fn(|_| Channel::new(partitions.len())),
            head,
            partitions,
            forward,
            inverse,
            work,
        }
    }

    /// Processes one sample of a channel, counted from 0. Channels beyond [`CHANNELS`] pass
    /// unchanged.
    pub fn process(&mut self, channel: usize, sample: f64) -> f64 {
        let Some(state) = self.channels.get_mut(channel) else {
            return sample;
        };
        #[allow(
            clippy::cast_possible_truncation,
            reason = "the filter runs with 32 bits, far below the noise of any headphones"
        )]
        let input = sample as f32;
        let position = state.position;
        for index in [position, position + BLOCK] {
            if let Some(slot) = state.recent.get_mut(index) {
                *slot = input;
            }
        }
        if let Some(slot) = state.input.get_mut(BLOCK + position) {
            *slot = input;
        }
        let recent = state
            .recent
            .get(position + 1..position + 1 + BLOCK)
            .unwrap_or_default();
        let head: f32 = self.head.iter().zip(recent).map(|(tap, x)| tap * x).sum();
        let output = head + state.tail.get(position).copied().unwrap_or(0.0);
        state.position += 1;
        if state.position == BLOCK {
            state.position = 0;
            Self::block(
                state,
                &self.partitions,
                self.forward.as_ref(),
                self.inverse.as_ref(),
                &mut self.work,
            );
        }
        f64::from(output)
    }

    /// At the end of a block: what the partitions add to the next one.
    fn block(
        state: &mut Channel,
        partitions: &[Vec<Complex<f32>>],
        forward: &dyn RealToComplex<f32>,
        inverse: &dyn ComplexToReal<f32>,
        work: &mut Work,
    ) {
        if partitions.is_empty() {
            return;
        }
        work.time.copy_from_slice(&state.input);
        state.input.copy_within(BLOCK.., 0);
        // The lengths are those of the plans, so these don't fail.
        drop(forward.process_with_scratch(
            &mut work.time,
            &mut work.spectrum,
            &mut work.scratch_forward,
        ));
        let count = state.spectra.len();
        state.newest = (state.newest + 1) % count;
        if let Some(newest) = state.spectra.get_mut(state.newest) {
            newest.copy_from_slice(&work.spectrum);
        }
        work.sum.fill(Complex::default());
        // Partition p meets the spectrum p blocks old.
        let spectra = (0..count)
            .map(|age| (state.newest + count - age) % count)
            .filter_map(|index| state.spectra.get(index));
        for (partition, spectrum) in partitions.iter().zip(spectra) {
            for ((sum, h), x) in work.sum.iter_mut().zip(partition).zip(spectrum) {
                *sum += h * x;
            }
        }
        for edge in [0, BLOCK] {
            if let Some(bin) = work.sum.get_mut(edge) {
                bin.im = 0.0;
            }
        }
        drop(inverse.process_with_scratch(
            &mut work.sum,
            &mut work.time,
            &mut work.scratch_inverse,
        ));
        state
            .tail
            .copy_from_slice(work.time.get(BLOCK..).unwrap_or_default());
    }

    /// Empties the state of each channel, e.g. when playback starts again.
    pub fn reset(&mut self) {
        let partitions = self.partitions.len();
        for channel in &mut self.channels {
            *channel = Channel::new(partitions);
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    reason = "silence and samples that pass are exact; the noise needs no precision"
)]
mod tests {
    use super::*;

    /// A sequence of numbers from -1 to 1 that looks random, the same in every run.
    fn noise(length: usize, seed: u32) -> Vec<f32> {
        let mut state = seed;
        (0..length)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 8) as f32 / (1 << 23) as f32 - 1.0
            })
            .collect()
    }

    fn direct(taps: &[f32], input: &[f32]) -> Vec<f64> {
        (0..input.len())
            .map(|n| {
                (0..taps.len().min(n + 1))
                    .map(|i| f64::from(taps[i]) * f64::from(input[n - i]))
                    .sum()
            })
            .collect()
    }

    #[test]
    fn convolves_like_the_definition() {
        for length in [1, 50, BLOCK, BLOCK + 1, 3 * BLOCK + 17, 1000] {
            let taps = noise(length, 7);
            let input = noise(2000, 11);
            let expected = direct(&taps, &input);
            let mut convolver = Convolver::new(&taps);
            for (n, (x, y)) in input.iter().zip(&expected).enumerate() {
                let output = convolver.process(1, f64::from(*x));
                assert!(
                    (output - y).abs() < 1e-3,
                    "{length} taps, sample {n}: {output} != {y}"
                );
            }
        }
    }

    #[test]
    fn channels_have_their_own_state_and_others_pass() {
        let taps = noise(500, 3);
        let mut convolver = Convolver::new(&taps);
        let left: Vec<f64> = (0..600)
            .map(|n| convolver.process(0, if n == 0 { 1.0 } else { 0.0 }))
            .collect();
        let right: Vec<f64> = (0..600).map(|_| convolver.process(1, 0.0)).collect();
        assert!((left[499] - f64::from(taps[499])).abs() < 1e-5);
        assert!(right.iter().all(|y| *y == 0.0));
        assert_eq!(convolver.process(2, 0.25), 0.25);
    }

    #[test]
    fn reset_empties_the_state() {
        let taps = noise(400, 5);
        let mut convolver = Convolver::new(&taps);
        let fresh = convolver.clone();
        for x in noise(300, 9) {
            convolver.process(0, f64::from(x));
        }
        assert_ne!(convolver, fresh);
        convolver.reset();
        assert_eq!(convolver, fresh);
        assert!(format!("{convolver:?}").starts_with("Convolver"));
    }
}
