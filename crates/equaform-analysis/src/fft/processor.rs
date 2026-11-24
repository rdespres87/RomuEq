//! Allocation-free FFT spectrum processor.

use super::windowing::{hamming_window, hann_window};
use super::{LogBinMode, SpectrumConfig, SpectrumFrame, WindowType};
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::Arc;

/// FFT processor that converts time-domain samples to spectrum data.
///
/// All buffers are pre-allocated at construction — no heap allocation
/// happens during `process()` calls, making it safe for the consumer
/// (GUI/worker) thread with predictable latency.
pub struct SpectrumProcessor {
    config: SpectrumConfig,
    fft_size: usize,
    num_bins: usize,
    /// Number of log groups (computed at construction).
    num_log_groups: usize,
    /// FFT planner instance.
    fft: Arc<dyn rustfft::Fft<f32>>,
    /// Hann or Hamming window coefficients.
    window: Vec<f32>,
    /// Previous spectrum for EMA smoothing.
    prev_magnitudes: Vec<f32>,
    /// True until the first `process()` call completes.
    first_frame: bool,
    /// Input buffer for windowed samples (f32, pre-windowing).
    input_buffer: Vec<f32>,
    /// Complex buffer for FFT in-place operation.
    complex_buffer: Vec<Complex<f32>>,
    /// Pre-allocated buffer for linear magnitudes.
    magnitudes: Vec<f32>,
    /// Pre-allocated buffer for dB magnitudes.
    magnitudes_db: Vec<f32>,
    /// Pre-allocated buffer for log group bin counting.
    log_group_counts: Vec<usize>,
    /// Pre-allocated reusable output frame.
    frame: SpectrumFrame,
}

impl std::fmt::Debug for SpectrumProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpectrumProcessor")
            .field("config", &self.config)
            .field("fft_size", &self.fft_size)
            .field("num_bins", &self.num_bins)
            .finish()
    }
}

impl SpectrumProcessor {
    /// Creates a new `SpectrumProcessor` with the given configuration.
    ///
    /// # Panics
    /// Panics if `fft_size` is not a power of 2.
    pub fn new(config: SpectrumConfig) -> Self {
        assert!(
            config.fft_size.is_power_of_two() && config.fft_size >= 2,
            "fft_size must be a power of 2 >= 2"
        );

        let fft_size = config.fft_size;
        let num_bins = fft_size / 2 + 1;

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);

        let window = match config.window_type {
            WindowType::Hann => hann_window(fft_size),
            WindowType::Hamming => hamming_window(fft_size),
        };

        let (log_linear, _, num_groups) = SpectrumFrame::empty_log_bins(&config);
        let _log_linear = log_linear; // keep for buffer sizing if needed

        let magnitudes = vec![0.0_f32; num_bins];
        let magnitudes_db = vec![config.min_db; num_bins];
        let log_group_counts = vec![0usize; num_groups];
        let frame = SpectrumFrame::empty(&config);

        Self {
            config,
            fft_size,
            num_bins,
            num_log_groups: num_groups,
            fft,
            window,
            first_frame: true,
            prev_magnitudes: vec![0.0_f32; num_bins],
            input_buffer: vec![0.0_f32; fft_size],
            complex_buffer: vec![Complex { re: 0.0, im: 0.0 }; fft_size],
            magnitudes,
            magnitudes_db,
            log_group_counts,
            frame,
        }
    }

    /// Returns a reference to the current configuration.
    pub fn config(&self) -> &SpectrumConfig {
        &self.config
    }

    /// Returns the FFT size.
    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    /// Returns the number of frequency bins.
    pub fn num_bins(&self) -> usize {
        self.num_bins
    }

    /// Reconfigure the processor. Buffers are reallocated if `fft_size` changes.
    pub fn reconfigure(&mut self, new_config: SpectrumConfig) {
        if new_config.fft_size != self.fft_size {
            let fft_size = new_config.fft_size;
            let num_bins = fft_size / 2 + 1;

            let mut planner = FftPlanner::<f32>::new();
            self.fft = planner.plan_fft_forward(fft_size);

            self.window = match new_config.window_type {
                WindowType::Hann => hann_window(fft_size),
                WindowType::Hamming => hamming_window(fft_size),
            };

            self.prev_magnitudes = vec![0.0_f32; num_bins];
            self.input_buffer = vec![0.0_f32; fft_size];
            self.complex_buffer = vec![Complex { re: 0.0, im: 0.0 }; fft_size];
            self.magnitudes = vec![0.0_f32; num_bins];
            self.magnitudes_db = vec![new_config.min_db; num_bins];
            self.fft_size = fft_size;
            self.num_bins = num_bins;

            // Recalculate log groups
            let (_, _, num_groups) = SpectrumFrame::empty_log_bins(&new_config);
            self.num_log_groups = num_groups;
            self.log_group_counts = vec![0usize; num_groups];
        }

        self.config = new_config;
        self.frame = SpectrumFrame::empty(&self.config);
    }

    /// Returns the number of log groups.
    pub fn num_log_groups(&self) -> usize {
        self.num_log_groups
    }

    /// Returns the center frequency of a bin.
    #[inline]
    fn bin_frequency(&self, sample_rate: f64, bin: usize) -> f64 {
        bin as f64 * sample_rate / self.fft_size as f64
    }

    /// Maps a frequency (Hz) to a log-group index.
    fn freq_to_log_group(&self, freq_hz: f64) -> usize {
        if freq_hz <= 0.0 {
            return 0;
        }

        match self.config.log_bin_mode {
            LogBinMode::None => 0,
            LogBinMode::Octave => {
                let base_freq = 25.5;
                (freq_hz / base_freq).log2().floor() as usize + 1
            }
            LogBinMode::OneThirdOctave => {
                let base_freq = 25.0;
                let idx = ((freq_hz / base_freq).log2() * 3.0).floor() as i64 + 10;
                idx.max(0) as usize
            }
        }
    }

    /// Processes a block of time-domain samples and produces a `SpectrumFrame`.
    ///
    /// # Arguments
    /// * `samples` — input samples (should have at least `fft_size` values).
    ///   If shorter, the remainder is zero-padded.
    /// * `sample_rate` — sample rate in Hz.
    /// * `frame_number` — sequence number forwarded from the source frame.
    pub fn process(
        &mut self,
        samples: &[f32],
        sample_rate: f64,
        frame_number: u64,
    ) -> SpectrumFrame {
        self.process_ref(samples, sample_rate, frame_number).clone()
    }

    /// Performs FFT analysis on the input samples with zero heap allocation,
    /// populating an existing `SpectrumFrame`.
    pub fn process_into(
        &mut self,
        samples: &[f32],
        sample_rate: f64,
        frame_number: u64,
        out: &mut SpectrumFrame,
    ) {
        let frame = self.process_ref(samples, sample_rate, frame_number);
        out.clone_from(frame);
    }

    /// Performs FFT analysis on the input samples with zero heap allocation,
    /// returning a reference to the internal reusable `SpectrumFrame`.
    pub fn process_ref(
        &mut self,
        samples: &[f32],
        sample_rate: f64,
        frame_number: u64,
    ) -> &SpectrumFrame {
        let fft_size = self.fft_size;

        // Copy and window the input.
        let len = samples.len().min(fft_size);
        for (i, (&sample, win)) in samples.iter().zip(self.window.iter()).take(len).enumerate() {
            self.input_buffer[i] = sample * win;
        }
        // Zero-pad the rest.
        for i in len..fft_size {
            self.input_buffer[i] = 0.0;
        }

        // Convert f32 -> Complex<f32> for FFT.
        for i in 0..fft_size {
            self.complex_buffer[i] = Complex {
                re: self.input_buffer[i],
                im: 0.0,
            };
        }

        // Perform FFT in-place.
        self.fft.process(&mut self.complex_buffer);

        // Compute magnitudes from complex FFT output into pre-allocated buffer.
        for (bin, comp) in self.complex_buffer.iter().take(self.num_bins).enumerate() {
            self.magnitudes[bin] = (comp.re * comp.re + comp.im * comp.im).sqrt();
        }

        // Apply equal-power normalization for window type.
        let window_energy = self.compute_window_energy();
        if window_energy > 0.0 {
            for m in self.magnitudes.iter_mut() {
                *m /= window_energy;
            }
        }

        // Apply EMA smoothing across frames (skip on first frame to avoid all-zero output).
        if self.first_frame {
            self.first_frame = false;
            self.prev_magnitudes.copy_from_slice(&self.magnitudes);
        } else {
            let alpha = self.config.smoothing_factor;
            for (prev, cur) in self.prev_magnitudes.iter_mut().zip(self.magnitudes.iter()) {
                *prev = alpha * cur + (1.0 - alpha) * (*prev);
            }
        }
        self.magnitudes.copy_from_slice(&self.prev_magnitudes);

        // Convert to dB.
        let min_db = self.config.min_db;
        for (bin, &m) in self.magnitudes.iter().enumerate() {
            let db = if m < 1e-30 { min_db } else { 20.0 * m.log10() };
            self.magnitudes_db[bin] = db.max(min_db);
        }

        // Compute log-binned magnitudes directly into self.frame.
        self.compute_log_bins_into(sample_rate);

        self.frame.sample_rate = sample_rate;
        self.frame.frame_number = frame_number;
        self.frame
            .magnitudes_linear
            .copy_from_slice(&self.magnitudes);
        self.frame
            .magnitudes_db
            .copy_from_slice(&self.magnitudes_db);
        self.frame.num_log_groups = self.num_log_groups;

        &self.frame
    }

    /// Computes the window energy for equal-power normalization.
    fn compute_window_energy(&self) -> f32 {
        let sum_sq: f32 = self.window.iter().map(|w| w * w).sum();
        (sum_sq / self.fft_size as f32).sqrt()
    }

    /// Groups linear bins into log-scaled frequency bands directly into `self.frame`.
    fn compute_log_bins_into(&mut self, sample_rate: f64) {
        if self.config.log_bin_mode == LogBinMode::None || self.num_log_groups == 0 {
            return;
        }

        let num_groups = self.num_log_groups;
        let min_db = self.config.min_db;

        self.frame.log_binned_magnitudes_linear.fill(0.0);
        self.log_group_counts.fill(0);

        for (bin, &mag) in self.magnitudes.iter().enumerate().take(self.num_bins) {
            let freq = self.bin_frequency(sample_rate, bin);
            let group = self.freq_to_log_group(freq);
            if group < num_groups {
                self.frame.log_binned_magnitudes_linear[group] += mag * mag;
                self.log_group_counts[group] += 1;
            }
        }

        for group in 0..num_groups {
            let count = self.log_group_counts[group];
            if count > 0 {
                let sum = self.frame.log_binned_magnitudes_linear[group];
                let lin = (sum / count as f32).sqrt();
                self.frame.log_binned_magnitudes_linear[group] = lin;
                let db = if lin < 1e-30 {
                    min_db
                } else {
                    20.0 * lin.log10()
                };
                self.frame.log_binned_magnitudes_db[group] = db.max(min_db);
            } else {
                self.frame.log_binned_magnitudes_linear[group] = 0.0;
                self.frame.log_binned_magnitudes_db[group] = min_db;
            }
        }
    }
}

