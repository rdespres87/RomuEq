//! Linear-phase FIR kernel synthesis from composite biquad magnitude curves.
//!
//! Computes zero-phase composite 2x2 magnitude matrices across all enabled EQ
//! bands (supporting Stereo, Left, Right, Mid, and Side per-band routing),
//! synthesizes centered Type-I symmetric FIR impulse responses via inverse FFT
//! and Blackman-Harris windowing, and transforms them into frequency-domain
//! UPOLS partition spectra.

use crate::coefficients::{BiquadCoefficients, FilterSpecification};
use equaform_common::{BandRouting, EqParameters, MAX_BANDS, ProcessingMode};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

/// Processing block size `B` for Uniformly Partitioned Overlap-Save (UPOLS).
pub const UPOLS_BLOCK_SIZE: usize = 256;

/// Complex FFT size `2 * B` used per UPOLS partition.
pub const UPOLS_FFT_SIZE: usize = UPOLS_BLOCK_SIZE * 2;

/// Synthesis FFT size for `ProcessingMode::LinearPhaseLow`.
pub const LINEAR_LOW_FFT_SIZE: usize = 2048;

/// FIR group delay `D_fir` (center tap index) for `ProcessingMode::LinearPhaseLow`.
///
/// With `N_fir = 2 * 768 + 1 = 1537` taps, total latency is `B + D_fir = 1024` samples.
pub const LINEAR_LOW_DELAY: usize = 768;

/// Synthesis FFT size for `ProcessingMode::LinearPhaseHigh`.
pub const LINEAR_HIGH_FFT_SIZE: usize = 4096;

/// FIR group delay `D_fir` (center tap index) for `ProcessingMode::LinearPhaseHigh`.
///
/// With `N_fir = 2 * 1792 + 1 = 3585` taps, total latency is `B + D_fir = 2048` samples.
pub const LINEAR_HIGH_DELAY: usize = 1792;

/// Maximum number of UPOLS frequency partitions allocated per bank.
pub const UPOLS_MAX_PARTITIONS: usize = 16;

const MAX_POSITIVE_BINS: usize = LINEAR_HIGH_FFT_SIZE / 2 + 1;

/// Evaluates the linear magnitude response `|H(e^{j*omega})|` of a biquad filter
/// at normalized angular frequency `omega` (radians/sample).
///
/// Exact at DC (`omega = 0.0`) and Nyquist (`omega = PI`). Returns `1.0` if any
/// coefficient or `omega` is non-finite, and clamps output to `[0.0, 100.0]`.
pub fn biquad_magnitude_linear(coeffs: &BiquadCoefficients, omega: f32) -> f32 {
    if !coeffs.is_finite() || !omega.is_finite() {
        return 1.0;
    }

    let w = f64::from(omega);
    let (sin_w, cos_w) = w.sin_cos();
    let (sin_2w, cos_2w) = (2.0 * w).sin_cos();

    let num_re =
        f64::from(coeffs.b0) + f64::from(coeffs.b1) * cos_w + f64::from(coeffs.b2) * cos_2w;
    let num_im = -f64::from(coeffs.b1) * sin_w - f64::from(coeffs.b2) * sin_2w;
    let num_sq = num_re * num_re + num_im * num_im;

    let den_re = 1.0 + f64::from(coeffs.a1) * cos_w + f64::from(coeffs.a2) * cos_2w;
    let den_im = -f64::from(coeffs.a1) * sin_w - f64::from(coeffs.a2) * sin_2w;
    let den_sq = den_re * den_re + den_im * den_im;

    if den_sq <= 1e-24 {
        return if num_sq <= 1e-24 { 1.0 } else { 100.0 };
    }

    let mag = (num_sq / den_sq).sqrt();
    if !mag.is_finite() {
        return 1.0;
    }

    (mag as f32).clamp(0.0, 100.0)
}

/// Evaluates the 4-term Blackman-Harris window at sample index `n` of length `len`.
///
/// For odd `len = 2 * D_fir + 1`, the center weight `blackman_harris(D_fir, len)`
/// equals `1.0`.
pub fn blackman_harris(n: usize, len: usize) -> f32 {
    if len <= 1 {
        return 1.0;
    }
    if n >= len {
        return 0.0;
    }

    let t = 2.0 * std::f64::consts::PI * (n as f64) / ((len - 1) as f64);
    let w = 0.35875 - 0.48829 * t.cos() + 0.14128 * (2.0 * t).cos() - 0.01168 * (3.0 * t).cos();
    (w as f32).clamp(0.0, 1.0)
}

/// Pre-allocated frequency-domain UPOLS partition spectra for a 2x2 stereo matrix.
#[derive(Clone, Debug)]
pub struct PartitionBank {
    /// Active partition count covering the synthesized FIR impulse response.
    pub num_partitions: usize,
    /// Whether off-diagonal cross terms (`lr` and `rl`) are active (Mid/Side bands present).
    pub has_cross_terms: bool,
    /// Left-to-left partition spectra (`UPOLS_MAX_PARTITIONS * UPOLS_FFT_SIZE`).
    pub ll: Vec<Complex<f32>>,
    /// Right-to-right partition spectra (`UPOLS_MAX_PARTITIONS * UPOLS_FFT_SIZE`).
    pub rr: Vec<Complex<f32>>,
    /// Right-to-left cross-term partition spectra (`UPOLS_MAX_PARTITIONS * UPOLS_FFT_SIZE`).
    pub lr: Vec<Complex<f32>>,
    /// Left-to-right cross-term partition spectra (`UPOLS_MAX_PARTITIONS * UPOLS_FFT_SIZE`).
    pub rl: Vec<Complex<f32>>,
}

impl PartitionBank {
    /// Creates a new zero-initialized partition bank with full capacity for 16 partitions.
    pub fn new() -> Self {
        let cap = UPOLS_MAX_PARTITIONS * UPOLS_FFT_SIZE;
        let zero = Complex::new(0.0, 0.0);
        Self {
            num_partitions: 1,
            has_cross_terms: false,
            ll: vec![zero; cap],
            rr: vec![zero; cap],
            lr: vec![zero; cap],
            rl: vec![zero; cap],
        }
    }

    /// Configures this bank as a single-partition unity passthrough kernel.
    fn set_unity(&mut self) {
        self.num_partitions = 1;
        self.has_cross_terms = false;
        self.ll[..UPOLS_FFT_SIZE].fill(Complex::new(1.0, 0.0));
        self.rr[..UPOLS_FFT_SIZE].fill(Complex::new(1.0, 0.0));
    }
}

impl Default for PartitionBank {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-allocated intermediate buffers for transforming one magnitude curve into UPOLS partitions.
struct SynthesisWorkspace {
    fft_part: Arc<dyn Fft<f32>>,
    spectrum: Vec<Complex<f32>>,
    fir_taps: Vec<f32>,
    part_buf: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
}

impl SynthesisWorkspace {
    fn synthesize_channel(
        &mut self,
        ifft: &Arc<dyn Fft<f32>>,
        magnitudes: &[f32],
        n_synth: usize,
        d_fir: usize,
        out_partitions: &mut [Complex<f32>],
    ) {
        let half_bins = n_synth / 2;
        let zero = Complex::new(0.0, 0.0);

        self.spectrum[0] = Complex::new(magnitudes[0], 0.0);
        for (k, &mag) in magnitudes.iter().enumerate().take(half_bins).skip(1) {
            let val = Complex::new(mag, 0.0);
            self.spectrum[k] = val;
            self.spectrum[n_synth - k] = val;
        }
        self.spectrum[half_bins] = Complex::new(magnitudes[half_bins], 0.0);

        ifft.process_with_scratch(&mut self.spectrum[..n_synth], &mut self.scratch);

        let n_fir = 2 * d_fir + 1;
        let inv_n_synth = 1.0 / n_synth as f32;
        for (n, tap) in self.fir_taps[..n_fir].iter_mut().enumerate() {
            let m = n as isize - d_fir as isize;
            let idx = m.rem_euclid(n_synth as isize) as usize;
            *tap = self.spectrum[idx].re * inv_n_synth * blackman_harris(n, n_fir);
        }

        for (p, chunk) in self.fir_taps[..n_fir].chunks(UPOLS_BLOCK_SIZE).enumerate() {
            for (dst, &src) in self.part_buf[..chunk.len()].iter_mut().zip(chunk.iter()) {
                *dst = Complex::new(src, 0.0);
            }
            self.part_buf[chunk.len()..UPOLS_FFT_SIZE].fill(zero);
            self.fft_part
                .process_with_scratch(&mut self.part_buf[..UPOLS_FFT_SIZE], &mut self.scratch);

            let dst_offset = p * UPOLS_FFT_SIZE;
            out_partitions[dst_offset..dst_offset + UPOLS_FFT_SIZE]
                .copy_from_slice(&self.part_buf[..UPOLS_FFT_SIZE]);
        }
    }
}

/// Zero-allocation linear-phase FIR kernel synthesizer.
///
/// Pre-allocates all FFT plans and scratch buffers at construction so
/// [`KernelSynthesizer::synthesize`] performs zero heap allocations.
pub struct KernelSynthesizer {
    ifft_low: Arc<dyn Fft<f32>>,
    ifft_high: Arc<dyn Fft<f32>>,
    mag_ll: Vec<f32>,
    mag_rr: Vec<f32>,
    mag_lr: Vec<f32>,
    mag_rl: Vec<f32>,
    workspace: SynthesisWorkspace,
}

impl std::fmt::Debug for KernelSynthesizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KernelSynthesizer")
            .field("low_fft_size", &LINEAR_LOW_FFT_SIZE)
            .field("high_fft_size", &LINEAR_HIGH_FFT_SIZE)
            .field("partition_fft_size", &UPOLS_FFT_SIZE)
            .finish()
    }
}

impl KernelSynthesizer {
    /// Creates a new synthesizer with all FFT plans and working buffers pre-allocated.
    pub fn new() -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let ifft_low = planner.plan_fft_inverse(LINEAR_LOW_FFT_SIZE);
        let ifft_high = planner.plan_fft_inverse(LINEAR_HIGH_FFT_SIZE);
        let fft_part = planner.plan_fft_forward(UPOLS_FFT_SIZE);

        let scratch_len = ifft_low
            .get_inplace_scratch_len()
            .max(ifft_high.get_inplace_scratch_len())
            .max(fft_part.get_inplace_scratch_len());

        let zero = Complex::new(0.0, 0.0);
        Self {
            ifft_low,
            ifft_high,
            mag_ll: vec![0.0; MAX_POSITIVE_BINS],
            mag_rr: vec![0.0; MAX_POSITIVE_BINS],
            mag_lr: vec![0.0; MAX_POSITIVE_BINS],
            mag_rl: vec![0.0; MAX_POSITIVE_BINS],
            workspace: SynthesisWorkspace {
                fft_part,
                spectrum: vec![zero; LINEAR_HIGH_FFT_SIZE],
                fir_taps: vec![0.0; LINEAR_HIGH_FFT_SIZE],
                part_buf: vec![zero; UPOLS_FFT_SIZE],
                scratch: vec![zero; scratch_len],
            },
        }
    }

    /// Synthesizes partitioned UPOLS frequency kernels from `params` into `out`
    /// without allocating on the heap.
    pub fn synthesize(
        &mut self,
        params: &EqParameters,
        sample_rate: f32,
        mode: ProcessingMode,
        out: &mut PartitionBank,
    ) {
        let (n_synth, d_fir, ifft) = match mode {
            ProcessingMode::MinimumPhase => {
                out.set_unity();
                return;
            }
            ProcessingMode::LinearPhaseLow => {
                (LINEAR_LOW_FFT_SIZE, LINEAR_LOW_DELAY, &self.ifft_low)
            }
            ProcessingMode::LinearPhaseHigh => {
                (LINEAR_HIGH_FFT_SIZE, LINEAR_HIGH_DELAY, &self.ifft_high)
            }
        };

        let n_fir = 2 * d_fir + 1;
        let num_partitions = n_fir.div_ceil(UPOLS_BLOCK_SIZE);

        let sr_valid = sample_rate.is_finite() && sample_rate > 0.0;
        let mut band_coeffs: [Option<BiquadCoefficients>; MAX_BANDS] = [None; MAX_BANDS];
        let mut has_cross_terms = false;

        if sr_valid {
            for (idx, (band, slot)) in params.bands.iter().zip(band_coeffs.iter_mut()).enumerate() {
                if band.enabled
                    && band.frequency_hz.is_finite()
                    && band.frequency_hz > 0.0
                    && band.gain_db.is_finite()
                    && band.q.is_finite()
                {
                    *slot = Some(FilterSpecification::from_band(band, sample_rate).compute());
                    if matches!(
                        params.band_routing(idx),
                        BandRouting::Mid | BandRouting::Side
                    ) {
                        has_cross_terms = true;
                    }
                }
            }
        }

        let half_bins = n_synth / 2;
        let omega_step = 2.0 * std::f32::consts::PI / n_synth as f32;

        for k in 0..=half_bins {
            let omega = k as f32 * omega_step;
            let (mut m_ll, mut m_lr, mut m_rl, mut m_rr) = (1.0_f32, 0.0_f32, 0.0_f32, 1.0_f32);

            for (idx, maybe_coeffs) in band_coeffs.iter().enumerate() {
                let Some(coeffs) = maybe_coeffs else {
                    continue;
                };
                let g = biquad_magnitude_linear(coeffs, omega);

                match params.band_routing(idx) {
                    BandRouting::Stereo => {
                        m_ll *= g;
                        m_lr *= g;
                        m_rl *= g;
                        m_rr *= g;
                    }
                    BandRouting::Left => {
                        m_ll *= g;
                        m_lr *= g;
                    }
                    BandRouting::Right => {
                        m_rl *= g;
                        m_rr *= g;
                    }
                    BandRouting::Mid => {
                        let a = 0.5 * (g + 1.0);
                        let b = 0.5 * (g - 1.0);
                        (m_ll, m_lr, m_rl, m_rr) = (
                            a * m_ll + b * m_rl,
                            a * m_lr + b * m_rr,
                            b * m_ll + a * m_rl,
                            b * m_lr + a * m_rr,
                        );
                    }
                    BandRouting::Side => {
                        let a = 0.5 * (g + 1.0);
                        let b = 0.5 * (1.0 - g);
                        (m_ll, m_lr, m_rl, m_rr) = (
                            a * m_ll + b * m_rl,
                            a * m_lr + b * m_rr,
                            b * m_ll + a * m_rl,
                            b * m_lr + a * m_rr,
                        );
                    }
                }
            }

            self.mag_ll[k] = m_ll;
            self.mag_rr[k] = m_rr;
            if has_cross_terms {
                self.mag_lr[k] = m_lr;
                self.mag_rl[k] = m_rl;
            }
        }

        out.num_partitions = num_partitions;
        out.has_cross_terms = has_cross_terms;

        self.workspace
            .synthesize_channel(ifft, &self.mag_ll, n_synth, d_fir, &mut out.ll);
        self.workspace
            .synthesize_channel(ifft, &self.mag_rr, n_synth, d_fir, &mut out.rr);
        if has_cross_terms {
            self.workspace
                .synthesize_channel(ifft, &self.mag_lr, n_synth, d_fir, &mut out.lr);
            self.workspace
                .synthesize_channel(ifft, &self.mag_rl, n_synth, d_fir, &mut out.rl);
        }
    }
}

impl Default for KernelSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}
