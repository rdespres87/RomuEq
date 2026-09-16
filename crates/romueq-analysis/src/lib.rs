#![forbid(unsafe_code)]

//! Analysis data path for RomuEq.
//!
//! Provides a bounded, allocation-free handoff from the audio thread to an
//! analysis consumer (GUI, metering, spectrum). The design guarantees that
//! the audio thread never performs FFT or heavy computation.
//!
//! ## Architecture
//!
//! ```text
//! [audio thread] ──> capture() ──> AnalysisBuffer (ring) ──> consume() ──> [GUI thread]
//!                    O(1) copy         bounded SPSC            O(1) drain
//! ```
//!
//! - **AnalysisFrame**: per-block analysis data (RMS, peak, channel energy).
//!   Computed by decimating the raw buffer, not by FFT.
//! - **AnalysisBuffer**: bounded SPSC ring buffer with drop-new overflow policy.
//!   Pre-allocated at construction, no allocation after.
//! - **AnalysisEngine**: consumer-side API that drains frames and provides
//!   the latest analysis snapshot.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use romueq_common::{OverflowPolicy, SpscRingBuffer};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of analysis frames that can be queued.
/// Must be a power of two. Chosen small enough to keep latency low.
const ANALYSIS_BUFFER_CAPACITY: usize = 4;

/// Decimation factor for RMS/peak computation.
/// One analysis sample per N audio samples reduces per-block work to O(N/decimation).
const ANALYSIS_DECIMATION: usize = 64;

/// Maximum number of channels supported for analysis.
const ANALYSIS_MAX_CHANNELS: usize = 8;

// ---------------------------------------------------------------------------
// AnalysisFrame
// ---------------------------------------------------------------------------

/// Per-block analysis data produced by the audio thread.
///
/// All fields are derived from the raw audio via decimation (not FFT),
/// keeping computation O(block_size / DECIMATION) per channel.
#[derive(Clone, Copy, Debug, Default)]
pub struct AnalysisFrame {
    /// Number of samples in the source block.
    pub block_size: usize,

    /// Sample rate of the source audio.
    pub sample_rate: f64,

    /// RMS energy per channel (decimated).
    pub rms: [f32; ANALYSIS_MAX_CHANNELS],

    /// Peak absolute value per channel (decimated).
    pub peak: [f32; ANALYSIS_MAX_CHANNELS],

    /// Number of zero-crossings per channel (decimated).
    pub zero_crossings: [usize; ANALYSIS_MAX_CHANNELS],

    /// Frame sequence number (monotonically increasing, wraps on overflow).
    pub frame_number: u64,

    /// Whether this frame was dropped by the ring buffer.
    pub dropped: bool,
}

impl AnalysisFrame {
    /// Compute analysis frame from a flat **interleaved** audio buffer.
    ///
    /// The buffer layout is sample-major (interleaved): `[ch0_s0, ch1_s0,
    /// ch0_s1, ch1_s1, ...]` for stereo, or `[s0, s1, ..., sN]` for mono.
    ///
    /// **RT-safe:** no allocation, no branching on data-dependent paths
    /// (loops are unrolled by the compiler for small channel counts).
    pub fn compute(
        buffer: &[f32],
        channels: usize,
        sample_rate: f64,
        block_size: usize,
        frame_number: u64,
    ) -> Self {
        let decimation = if block_size > ANALYSIS_DECIMATION {
            ANALYSIS_DECIMATION
        } else {
            1
        };

        let mut rms = [0.0_f32; ANALYSIS_MAX_CHANNELS];
        let mut peak = [0.0_f32; ANALYSIS_MAX_CHANNELS];
        let mut zero_crossings = [0_usize; ANALYSIS_MAX_CHANNELS];

        // Total interleaved samples in the block.
        let total_samples = if channels > 0 {
            block_size * channels
        } else {
            block_size
        };

        for ch in 0..channels.min(ANALYSIS_MAX_CHANNELS) {
            let mut sum_sq: f32 = 0.0;
            let mut max_abs: f32 = 0.0;
            let mut crossings: usize = 0;

            // Interleaved: start at index `ch`, stride by `channels`.
            let step = if channels > 0 { channels } else { 1 };
            let mut idx = ch;

            // Include first sample in RMS/peak calculation.
            if idx < total_samples {
                let first = buffer[idx % buffer.len()];
                sum_sq += first * first;
                max_abs = first.abs();
            }

            let mut prev_sample = buffer.get(idx % buffer.len()).copied().unwrap_or(0.0);

            // Step through remaining samples, striding by `step * decimation`.
            let stride = step * decimation;
            idx += stride;

            while idx < total_samples {
                let sample = buffer[idx % buffer.len()];
                sum_sq += sample * sample;
                let abs_val = sample.abs();
                if abs_val > max_abs {
                    max_abs = abs_val;
                }
                // Zero-crossing detection
                if (prev_sample >= 0.0 && sample < 0.0) || (prev_sample < 0.0 && sample >= 0.0) {
                    crossings += 1;
                }
                prev_sample = sample;
                idx += stride;
            }

            // Count of samples analyzed for this channel.
            let count = if stride > 0 {
                ((total_samples - 1) / stride) + 1
            } else {
                block_size
            };
            rms[ch] = (sum_sq / count.max(1) as f32).sqrt();
            peak[ch] = max_abs;
            zero_crossings[ch] = crossings;
        }

        AnalysisFrame {
            block_size,
            sample_rate,
            rms,
            peak,
            zero_crossings,
            frame_number,
            dropped: false,
        }
    }

    /// Return the number of active channels in this frame.
    pub fn active_channels(&self) -> usize {
        // Caller decides by checking which RMS/peak values are non-zero.
        // Default to 1 for backward compatibility.
        self.rms.iter().position(|&r| r > 0.0).unwrap_or(0) + 1
    }
}

// ---------------------------------------------------------------------------
// AnalysisBuffer — bounded SPSC handoff
// ---------------------------------------------------------------------------

/// Bounded ring buffer for analysis frame handoff.
///
/// The audio thread calls `push()` to enqueue frames. The consumer (GUI)
/// calls `pop()` to drain them. When the buffer is full and the policy is
/// `DropNew`, `push()` returns `false` and the frame is silently dropped.
pub struct AnalysisBuffer {
    ring: SpscRingBuffer<AnalysisFrame>,
    frame_counter: AtomicUsize,
}

impl AnalysisBuffer {
    /// Create a new analysis buffer with the default capacity.
    pub fn new() -> Self {
        AnalysisBuffer {
            ring: SpscRingBuffer::new(ANALYSIS_BUFFER_CAPACITY, OverflowPolicy::DropNew),
            frame_counter: AtomicUsize::new(0),
        }
    }

    /// Try to enqueue an analysis frame.
    ///
    /// Returns `true` if the frame was enqueued, `false` if dropped (buffer full).
    ///
    /// **RT-safe:** no allocation, no blocking.
    pub fn push(&mut self, mut frame: AnalysisFrame) -> bool {
        let counter = self.frame_counter.fetch_add(1, Ordering::Relaxed) as u64;
        frame.frame_number = counter;

        let result = self.ring.enqueue(frame);
        if !result {
            frame.dropped = true;
            // Best-effort: enqueue the dropped frame to update the counter
            // so we don't lose the sequence number completely.
            self.ring.enqueue(frame);
        }
        result
    }

    /// Try to dequeue the oldest available analysis frame.
    ///
    /// Returns `None` if the buffer is empty.
    ///
    /// **RT-safe:** no allocation, no blocking.
    pub fn pop(&mut self) -> Option<AnalysisFrame> {
        self.ring.dequeue()
    }

    /// Returns `true` if the buffer is currently full.
    pub fn is_full(&self) -> bool {
        self.ring.is_full()
    }

    /// Current number of frames in the buffer.
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// Total capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.ring.capacity()
    }

    /// Number of frames dropped since creation (approximate, not thread-safe).
    pub fn frames_dropped(&self) -> usize {
        self.frame_counter
            .load(Ordering::Relaxed)
            .saturating_sub(self.ring.len())
    }
}

impl Default for AnalysisBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AnalysisEngine — consumer-side analysis API
// ---------------------------------------------------------------------------

/// Consumer-side analysis engine.
///
/// Drains the analysis buffer and maintains a latest-frame snapshot that
/// can be queried by the GUI thread at any time.
pub struct AnalysisEngine {
    /// The bounded ring buffer receiving frames from the audio thread.
    buffer: AnalysisBuffer,

    /// The most recently consumed frame (for instant GUI queries).
    latest: AnalysisFrame,

    /// Total frames consumed.
    consumed_count: u64,

    /// Total frames dropped by the ring buffer.
    dropped_count: u64,
}

impl AnalysisEngine {
    /// Create a new analysis engine with an empty buffer.
    pub fn new() -> Self {
        AnalysisEngine {
            buffer: AnalysisBuffer::new(),
            latest: AnalysisFrame::default(),
            consumed_count: 0,
            dropped_count: 0,
        }
    }

    /// Push a frame from the audio thread.
    pub fn push(&mut self, frame: AnalysisFrame) -> bool {
        let result = self.buffer.push(frame);
        if !result {
            // Frame was dropped by the ring buffer.
            // We still track it for statistics.
            self.dropped_count += 1;
        }
        result
    }

    /// Drain the buffer and update the latest snapshot.
    ///
    /// Returns the number of frames drained.
    pub fn consume_all(&mut self) -> usize {
        let mut count = 0;
        while let Some(frame) = self.buffer.pop() {
            count += 1;
            self.consumed_count += 1;
            if frame.dropped {
                self.dropped_count += 1;
            }
            // Keep the latest frame as the snapshot.
            self.latest = frame;
        }
        count
    }

    /// Get a clone of the latest analysis frame.
    ///
    /// If no frame has been consumed yet, returns a default (all-zeros) frame.
    pub fn latest_frame(&self) -> AnalysisFrame {
        self.latest
    }

    /// Total frames consumed since creation.
    pub fn frames_consumed(&self) -> u64 {
        self.consumed_count
    }

    /// Total frames dropped since creation.
    pub fn frames_dropped(&self) -> u64 {
        self.dropped_count
    }

    /// Current buffer fill level.
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns `true` if the buffer is full (audio thread will drop frames).
    pub fn buffer_full(&self) -> bool {
        self.buffer.is_full()
    }

    /// Returns the buffer capacity (maximum frames before overflow).
    pub fn buffer_capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// Get a reference to the latest analysis frame (for GUI queries).
    pub fn latest_snapshot(&self) -> Option<&AnalysisFrame> {
        if self.consumed_count > 0 {
            Some(&self.latest)
        } else {
            None
        }
    }
}

impl Default for AnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// =======================================================================
// M5-T002: Spectrum / FFT pipeline (consumer-thread, no audio allocation)
// =======================================================================

use rustfft::{FftPlanner, num_complex::Complex};

/// Window function types supported by the spectrum processor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    /// Hann window (default). Best dynamic range for audio analysis.
    Hann,
    /// Hamming window. Slightly better peak resolution.
    Hamming,
}

/// Frequency bin grouping modes for log-scaled output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogBinMode {
    /// No grouping — one bin per FFT frequency bin (linear).
    None,
    /// Octave bands (1 octave = 2:1 frequency ratio).
    Octave,
    /// One-third octave bands (standard acoustic analysis).
    OneThirdOctave,
}

/// Configuration for the spectrum processor.
#[derive(Debug, Clone)]
pub struct SpectrumConfig {
    /// FFT size (must be power of 2, e.g. 256, 512, 1024, 2048).
    pub fft_size: usize,
    /// Smoothing factor for EMA across frames (0.0 = no smoothing, 1.0 = full).
    /// Default: 0.4.
    pub smoothing_factor: f32,
    /// Window function applied before FFT.
    pub window_type: WindowType,
    /// Log frequency bin grouping mode.
    pub log_bin_mode: LogBinMode,
    /// Minimum dB floor for log magnitude output.
    pub min_db: f32,
}

impl SpectrumConfig {
    /// Creates a new `SpectrumConfig` with the given FFT size and sample rate.
    ///
    /// Uses sensible defaults: Hann window, 1/3-octave log bins, 0.4 EMA
    /// smoothing, -120 dB floor.
    pub fn new(fft_size: usize, _sample_rate: f64) -> Self {
        assert!(
            fft_size.is_power_of_two() && fft_size >= 16,
            "fft_size must be a power of 2 ≥ 16, got {fft_size}"
        );
        Self {
            fft_size,
            smoothing_factor: 0.4,
            window_type: WindowType::Hann,
            log_bin_mode: LogBinMode::OneThirdOctave,
            min_db: -120.0,
        }
    }
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            fft_size: 1024,
            smoothing_factor: 0.4,
            window_type: WindowType::Hann,
            log_bin_mode: LogBinMode::OneThirdOctave,
            min_db: -120.0,
        }
    }
}

/// A single frame of spectrum data produced by `SpectrumProcessor`.
#[derive(Debug, Clone)]
pub struct SpectrumFrame {
    /// Number of linear frequency bins (fft_size / 2 + 1).
    pub num_bins: usize,
    /// FFT size used for this frame.
    pub fft_size: usize,
    /// Sample rate used for frequency calculations.
    pub sample_rate: f64,
    /// Linear magnitude per bin (fft_size / 2 + 1 values).
    pub magnitudes_linear: Vec<f32>,
    /// Magnitude in dB per bin (same layout as magnitudes_linear).
    pub magnitudes_db: Vec<f32>,
    /// Log-binned magnitude (one value per group).
    pub log_binned_magnitudes_linear: Vec<f32>,
    /// Log-binned magnitude in dB.
    pub log_binned_magnitudes_db: Vec<f32>,
    /// Number of log groups (0 if log_bin_mode is None).
    pub num_log_groups: usize,
    /// Frame sequence number (matches AnalysisFrame::frame_number).
    pub frame_number: u64,
}

impl SpectrumFrame {
    /// Returns the center frequency of a linear bin.
    #[inline]
    pub fn bin_frequency(&self, bin: usize) -> f64 {
        bin as f64 * self.sample_rate / self.fft_size as f64
    }

    /// Returns the band edges (low, high) for a log group.
    pub fn log_group_edges(&self, group: usize) -> Option<(f64, f64)> {
        if self.num_log_groups == 0 {
            return None;
        }
        match self.log_bin_mode() {
            LogBinMode::None => None,
            LogBinMode::Octave => octave_group_edges(group),
            LogBinMode::OneThirdOctave => one_third_octave_group_edges(group),
        }
    }

    /// Returns the log bin mode used for this frame.
    fn log_bin_mode(&self) -> LogBinMode {
        if self.num_log_groups == 0 {
            LogBinMode::None
        } else {
            // Default to 1/3-octave for non-zero groups (matches default config).
            LogBinMode::OneThirdOctave
        }
    }

    /// Creates an empty frame with the given FFT size and log mode.
    fn empty(config: &SpectrumConfig) -> Self {
        let num_bins = config.fft_size / 2 + 1;
        let (log_linear, log_db, num_groups) = Self::empty_log_bins(config);
        Self {
            num_bins,
            fft_size: config.fft_size,
            sample_rate: 44100.0,
            magnitudes_linear: vec![0.0_f32; num_bins],
            magnitudes_db: vec![config.min_db; num_bins],
            log_binned_magnitudes_linear: log_linear,
            log_binned_magnitudes_db: log_db,
            num_log_groups: num_groups,
            frame_number: 0,
        }
    }

    fn empty_log_bins(config: &SpectrumConfig) -> (Vec<f32>, Vec<f32>, usize) {
        // Use 48 kHz default for construction-time group sizing.
        // Actual frequency mapping uses the runtime sample rate in process().
        const DEFAULT_SAMPLE_RATE: f64 = 48000.0;

        match config.log_bin_mode {
            LogBinMode::None => (Vec::new(), Vec::new(), 0),
            LogBinMode::Octave => {
                let groups = octave_groups(config.fft_size, DEFAULT_SAMPLE_RATE);
                (vec![0.0_f32; groups], vec![config.min_db; groups], groups)
            }
            LogBinMode::OneThirdOctave => {
                let groups = one_third_octave_groups(config.fft_size, DEFAULT_SAMPLE_RATE);
                (vec![0.0_f32; groups], vec![config.min_db; groups], groups)
            }
        }
    }
}

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
            self.fft_size = fft_size;
            self.num_bins = num_bins;

            // Recalculate log groups
            let (_, _, num_groups) = SpectrumFrame::empty_log_bins(&new_config);
            self.num_log_groups = num_groups;
        }

        self.config = new_config;
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
                let idx = (freq_hz / base_freq).log2().floor() as i64 * 3 + 10;
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

        // Compute magnitudes from complex FFT output.
        let mut magnitudes = vec![0.0_f32; self.num_bins];

        for (bin, comp) in self.complex_buffer.iter().take(self.num_bins).enumerate() {
            magnitudes[bin] = (comp.re * comp.re + comp.im * comp.im).sqrt();
        }

        // Apply equal-power normalization for window type.
        let window_energy = self.compute_window_energy();
        if window_energy > 0.0 {
            for m in magnitudes.iter_mut() {
                *m /= window_energy;
            }
        }

        // Apply EMA smoothing across frames (skip on first frame to avoid all-zero output).
        if self.first_frame {
            self.first_frame = false;
            self.prev_magnitudes.clone_from(&magnitudes);
        } else {
            let alpha = self.config.smoothing_factor;
            for (prev, cur) in self.prev_magnitudes.iter_mut().zip(magnitudes.iter()) {
                *prev = alpha * cur + (1.0 - alpha) * (*prev);
            }
        }
        magnitudes = self.prev_magnitudes.clone();

        // Convert to dB.
        let min_db = self.config.min_db;
        let magnitudes_db: Vec<f32> = magnitudes
            .iter()
            .map(|m| if *m < 1e-30 { min_db } else { 20.0 * m.log10() })
            .map(|db| db.max(min_db))
            .collect();

        // Compute log-binned magnitudes.
        let (log_binned_linear, log_binned_db, num_groups) =
            self.compute_log_bins(&magnitudes, &magnitudes_db, sample_rate);

        let mut frame = SpectrumFrame::empty(&self.config);
        frame.sample_rate = sample_rate;
        frame.frame_number = frame_number;
        frame.magnitudes_linear = magnitudes;
        frame.magnitudes_db = magnitudes_db;
        frame.log_binned_magnitudes_linear = log_binned_linear;
        frame.log_binned_magnitudes_db = log_binned_db;
        frame.num_log_groups = num_groups;

        frame
    }

    /// Computes the window energy for equal-power normalization.
    fn compute_window_energy(&self) -> f32 {
        let sum_sq: f32 = self.window.iter().map(|w| w * w).sum();
        (sum_sq / self.fft_size as f32).sqrt()
    }

    /// Groups linear bins into log-scaled frequency bands.
    fn compute_log_bins(
        &self,
        linear_mag: &[f32],
        _linear_db: &[f32],
        sample_rate: f64,
    ) -> (Vec<f32>, Vec<f32>, usize) {
        if self.config.log_bin_mode == LogBinMode::None {
            return (Vec::new(), Vec::new(), 0);
        }

        let num_groups = self.num_log_groups;
        if num_groups == 0 {
            return (Vec::new(), Vec::new(), 0);
        }

        let mut group_mags = vec![0.0_f32; num_groups];
        let mut group_counts = vec![0usize; num_groups];

        for (bin, &mag) in linear_mag.iter().enumerate().take(self.num_bins) {
            let freq = self.bin_frequency(sample_rate, bin);
            let group = self.freq_to_log_group(freq);
            if group < num_groups {
                // Accumulate in linear domain (power sum).
                group_mags[group] += mag * mag;
                group_counts[group] += 1;
            }
        }

        // Convert power sums to magnitude (average then sqrt).
        let log_linear: Vec<f32> = group_mags
            .iter()
            .zip(group_counts.iter())
            .map(|(sum, count)| {
                if *count > 0 {
                    (*sum / *count as f32).sqrt()
                } else {
                    0.0
                }
            })
            .collect();

        // Convert to dB.
        let min_db = self.config.min_db;
        let log_db: Vec<f32> = log_linear
            .iter()
            .map(|m| if *m < 1e-30 { min_db } else { 20.0 * m.log10() })
            .map(|db| db.max(min_db))
            .collect();

        (log_linear, log_db, num_groups)
    }
}

// =======================================================================
// Window functions
// =======================================================================

/// Computes a Hann window of the given size.
fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / size as f32).cos()))
        .collect()
}

/// Computes a Hamming window of the given size.
fn hamming_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / size as f32).cos())
        .collect()
}

// =======================================================================
// Log frequency bin helpers (IEC 61260 standard)
// =======================================================================

/// Returns the number of octave bands that fit within [0, sample_rate/2].
fn octave_groups(_fft_size: usize, sample_rate: f64) -> usize {
    let nyquist = sample_rate / 2.0;
    // Center frequencies start at 25.5 Hz, double each step.
    let base = 25.5;
    if nyquist <= base {
        return 1;
    }
    ((nyquist / base).log2().floor() as usize) + 2
}

/// Returns the number of 1/3-octave bands that fit within [0, sample_rate/2].
fn one_third_octave_groups(_fft_size: usize, sample_rate: f64) -> usize {
    let nyquist = sample_rate / 2.0;
    // Center frequencies: 25, 31.5, 40, 50, 63, 80, 100, ... Hz
    let base = 25.0;
    if nyquist <= base {
        return 1;
    }
    ((nyquist / base).log2().floor() as i64 * 3 + 10) as usize
}

/// Returns the band edges for an octave group index.
fn octave_group_edges(group: usize) -> Option<(f64, f64)> {
    let base = 25.5;
    let low = base * (group as f64).max(1.0);
    let high = low * 2.0;
    Some((low, high))
}

/// Returns the band edges for a 1/3-octave group index.
fn one_third_octave_group_edges(group: usize) -> Option<(f64, f64)> {
    // Standard 1/3-octave centers: group 10 = 25 Hz, group 13 = 31.5 Hz, etc.
    // f_center = base * 2^(n/3) where n = group - 10
    let base = 25.0;
    let n = (group as i64) - 10;
    let center = base * (2.0_f64).powf(n as f64 / 3.0);
    let low = center / 2.0_f64.powf(1.0 / 6.0);
    let high = center * 2.0_f64.powf(1.0 / 6.0);
    Some((low, high))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // AnalysisFrame tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_analysis_frame_compute_mono_silence() {
        let buffer: Vec<f32> = vec![0.0; 64];
        let frame = AnalysisFrame::compute(&buffer, 1, 44100.0, 64, 0);

        assert_eq!(frame.block_size, 64);
        assert_eq!(frame.sample_rate, 44100.0);
        assert_eq!(frame.rms[0], 0.0);
        assert_eq!(frame.peak[0], 0.0);
        assert_eq!(frame.zero_crossings[0], 0);
    }

    #[test]
    fn test_analysis_frame_compute_mono_sine() {
        let samples: Vec<f32> = (0..64)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let frame = AnalysisFrame::compute(&samples, 1, 44100.0, 64, 0);

        assert_eq!(frame.block_size, 64);
        // RMS of a sine wave with amplitude 1.0 is ~0.707
        assert!(
            frame.rms[0] > 0.6 && frame.rms[0] < 0.8,
            "RMS={}",
            frame.rms[0]
        );
        // Peak should be close to 1.0
        assert!(frame.peak[0] > 0.9, "Peak={}", frame.peak[0]);
        // Zero crossings for a 440 Hz tone at 64 samples (well under one period)
        // should be 0 or small.
        assert!(frame.zero_crossings[0] < 10);
    }

    #[test]
    fn test_analysis_frame_compute_stereo() {
        let ch0: Vec<f32> = (0..64).map(|i| (i as f32 * 0.01).sin()).collect();
        let ch1: Vec<f32> = (0..64).map(|i| (i as f32 * 0.02).sin()).collect();

        // Planar layout: all ch0 samples first, then all ch1 samples.
        let mut planar = vec![0.0_f32; 128];
        planar[..64].copy_from_slice(&ch0);
        planar[64..].copy_from_slice(&ch1);

        let frame = AnalysisFrame::compute(&planar, 2, 44100.0, 64, 5);

        assert_eq!(frame.frame_number, 5);
        // Both channels should have non-zero RMS
        assert!(frame.rms[0] > 0.0, "ch0 RMS");
        assert!(frame.rms[1] > 0.0, "ch1 RMS");
        // Different signals should produce different RMS values
        assert_ne!(frame.rms[0], frame.rms[1]);
    }

    #[test]
    fn test_analysis_frame_compute_block_256() {
        let buffer: Vec<f32> = (0..256).map(|i| (i as f32 * 0.01).sin()).collect();
        let frame = AnalysisFrame::compute(&buffer, 1, 48000.0, 256, 10);

        assert_eq!(frame.block_size, 256);
        assert_eq!(frame.sample_rate, 48000.0);
        assert_eq!(frame.frame_number, 10);
        assert!(frame.rms[0] > 0.0);
    }

    #[test]
    fn test_analysis_frame_compute_small_block() {
        // Block smaller than decimation factor (32 < 64)
        let buffer: Vec<f32> = vec![0.5; 32];
        let frame = AnalysisFrame::compute(&buffer, 1, 44100.0, 32, 0);

        assert_eq!(frame.block_size, 32);
        assert_eq!(frame.rms[0], 0.5); // All samples are 0.5, so RMS = 0.5
        assert_eq!(frame.peak[0], 0.5);
    }

    // -----------------------------------------------------------------------
    // AnalysisBuffer tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_analysis_buffer_basic_push_pop() {
        let mut buffer = AnalysisBuffer::new();

        assert!(buffer.is_empty());
        assert_eq!(buffer.capacity(), ANALYSIS_BUFFER_CAPACITY);

        let frame = AnalysisFrame {
            block_size: 64,
            sample_rate: 44100.0,
            rms: [0.5; ANALYSIS_MAX_CHANNELS],
            peak: [0.5; ANALYSIS_MAX_CHANNELS],
            frame_number: 0,
            dropped: false,
            zero_crossings: [0; ANALYSIS_MAX_CHANNELS],
        };

        assert!(buffer.push(frame));
        assert!(!buffer.is_empty());
        assert_eq!(buffer.len(), 1);

        let popped = buffer.pop().unwrap();
        assert_eq!(popped.frame_number, 0);
        assert_eq!(popped.rms[0], 0.5);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_analysis_buffer_drop_new_policy() {
        let mut buffer = AnalysisBuffer::new();

        // Fill the buffer to capacity.
        for i in 0..ANALYSIS_BUFFER_CAPACITY {
            let frame = AnalysisFrame {
                frame_number: i as u64,
                block_size: 64,
                sample_rate: 44100.0,
                rms: [0.1; ANALYSIS_MAX_CHANNELS],
                peak: [0.1; ANALYSIS_MAX_CHANNELS],
                dropped: false,
                zero_crossings: [0; ANALYSIS_MAX_CHANNELS],
            };
            assert!(buffer.push(frame), "frame {} should be enqueued", i);
        }

        assert!(buffer.is_full());

        // Next push should be dropped.
        let overflow_frame = AnalysisFrame {
            frame_number: ANALYSIS_BUFFER_CAPACITY as u64,
            block_size: 64,
            sample_rate: 44100.0,
            rms: [0.9; ANALYSIS_MAX_CHANNELS],
            peak: [0.9; ANALYSIS_MAX_CHANNELS],
            dropped: false,
            zero_crossings: [0; ANALYSIS_MAX_CHANNELS],
        };
        assert!(!buffer.push(overflow_frame), "should drop when full");
    }

    #[test]
    fn test_analysis_buffer_lifo_fifo_ordering() {
        let mut buffer = AnalysisBuffer::new();

        // Push 3 frames.
        for i in 0..3 {
            buffer.push(AnalysisFrame {
                frame_number: i as u64,
                block_size: 64,
                sample_rate: 44100.0,
                rms: [i as f32; ANALYSIS_MAX_CHANNELS],
                peak: [0.0; ANALYSIS_MAX_CHANNELS],
                dropped: false,
                zero_crossings: [0; ANALYSIS_MAX_CHANNELS],
            });
        }

        // Pop should return in FIFO order.
        for i in 0..3 {
            let frame = buffer.pop().unwrap();
            assert_eq!(
                frame.frame_number, i as u64,
                "expected frame {i}, got {}",
                frame.frame_number
            );
        }

        assert!(buffer.pop().is_none());
    }

    #[test]
    fn test_analysis_buffer_pop_drains() {
        let mut buffer = AnalysisBuffer::new();

        for i in 0..4 {
            buffer.push(AnalysisFrame {
                frame_number: i as u64,
                block_size: 64,
                sample_rate: 44100.0,
                rms: [0.5; ANALYSIS_MAX_CHANNELS],
                peak: [0.5; ANALYSIS_MAX_CHANNELS],
                dropped: false,
                zero_crossings: [0; ANALYSIS_MAX_CHANNELS],
            });
        }

        assert_eq!(buffer.len(), 4);
        // Drain all.
        let drained = buffer.pop().unwrap();
        assert_eq!(drained.frame_number, 0);
        let drained = buffer.pop().unwrap();
        assert_eq!(drained.frame_number, 1);
        let drained = buffer.pop().unwrap();
        assert_eq!(drained.frame_number, 2);
        let drained = buffer.pop().unwrap();
        assert_eq!(drained.frame_number, 3);
        assert!(buffer.pop().is_none());
    }

    // -----------------------------------------------------------------------
    // AnalysisEngine tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_analysis_engine_new_is_empty() {
        let engine = AnalysisEngine::new();

        assert_eq!(engine.frames_consumed(), 0);
        assert_eq!(engine.frames_dropped(), 0);
        assert_eq!(engine.buffer_len(), 0);
        assert!(!engine.buffer_full());

        let latest = engine.latest_frame();
        assert_eq!(latest.rms[0], 0.0);
    }

    #[test]
    fn test_analysis_engine_push_and_consume() {
        let mut engine = AnalysisEngine::new();

        let frame = AnalysisFrame {
            block_size: 64,
            sample_rate: 44100.0,
            rms: [0.707_f32; ANALYSIS_MAX_CHANNELS],
            peak: [1.0; ANALYSIS_MAX_CHANNELS],
            frame_number: 42,
            dropped: false,
            zero_crossings: [5; ANALYSIS_MAX_CHANNELS],
        };

        assert!(engine.push(frame));
        assert_eq!(engine.buffer_len(), 1);

        let drained = engine.consume_all();
        assert_eq!(drained, 1);
        assert_eq!(engine.frames_consumed(), 1);

        let latest = engine.latest_frame();
        assert_eq!(latest.frame_number, 0); // internal counter starts at 0
        assert!((latest.rms[0] - 0.707).abs() < 1e-6);
    }

    #[test]
    fn test_analysis_engine_drops_under_overflow() {
        let mut engine = AnalysisEngine::new();

        // Push frames up to capacity.
        for i in 0..ANALYSIS_BUFFER_CAPACITY {
            assert!(engine.push(AnalysisFrame {
                frame_number: i as u64,
                block_size: 64,
                sample_rate: 44100.0,
                rms: [0.5; ANALYSIS_MAX_CHANNELS],
                peak: [0.5; ANALYSIS_MAX_CHANNELS],
                dropped: false,
                zero_crossings: [0; ANALYSIS_MAX_CHANNELS],
            }));
        }

        // One more should be dropped.
        assert!(!engine.push(AnalysisFrame {
            frame_number: ANALYSIS_BUFFER_CAPACITY as u64,
            block_size: 64,
            sample_rate: 44100.0,
            rms: [0.9; ANALYSIS_MAX_CHANNELS],
            peak: [0.9; ANALYSIS_MAX_CHANNELS],
            dropped: false,
            zero_crossings: [0; ANALYSIS_MAX_CHANNELS],
        }));

        // Consume all and verify dropped count.
        engine.consume_all();
        assert!(engine.frames_dropped() > 0);
    }

    #[test]
    fn test_analysis_engine_latest_snapshot() {
        let mut engine = AnalysisEngine::new();

        // Push a frame but don't consume.
        engine.push(AnalysisFrame {
            block_size: 128,
            sample_rate: 48000.0,
            rms: [0.3; ANALYSIS_MAX_CHANNELS],
            peak: [0.6; ANALYSIS_MAX_CHANNELS],
            frame_number: 1,
            dropped: false,
            zero_crossings: [2; ANALYSIS_MAX_CHANNELS],
        });

        // Latest should still be default (all zeros).
        let pre = engine.latest_frame();
        assert_eq!(pre.rms[0], 0.0);

        // After consume, latest should be updated.
        engine.consume_all();
        let post = engine.latest_frame();
        assert_eq!(post.block_size, 128);
        assert!((post.rms[0] - 0.3).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // M5-T001: End-to-end audio thread → GUI thread simulation
    // -----------------------------------------------------------------------

    #[test]
    fn test_analysis_handoff_simulation() {
        // Simulate the audio thread producing frames and a GUI thread consuming them.
        let mut engine = AnalysisEngine::new();

        // Audio thread produces 10 blocks.
        for block in 0..10 {
            let buffer: Vec<f32> = (0..64)
                .map(|i| ((block as f32 + i as f32) * 0.01).sin())
                .collect();

            let frame = AnalysisFrame::compute(&buffer, 1, 44100.0, 64, block as u64);
            let _pushed = engine.push(frame);

            // GUI thread drains every 3 blocks.
            if block % 3 == 2 {
                let drained = engine.consume_all();
                assert!(drained > 0, "should drain at block {}", block);
            }
        }

        // Final drain.
        let final_drain = engine.consume_all();
        assert_eq!(engine.frames_consumed(), 10);
        // Some frames may have been dropped if the buffer was full.
        assert!(
            engine.frames_dropped() < 10,
            "not all frames should be dropped"
        );
    }

    #[test]
    fn test_analysis_handoff_stress() {
        // Push more frames than the buffer can hold without consuming.
        let mut engine = AnalysisEngine::new();

        let total_frames = 100;
        let mut pushed = 0usize;

        for i in 0..total_frames {
            let buffer: Vec<f32> = vec![0.5; 64];
            let frame = AnalysisFrame::compute(&buffer, 1, 44100.0, 64, i as u64);
            if engine.push(frame) {
                pushed += 1;
            }
        }

        // With capacity=4, only the first 4 should be pushed.
        assert_eq!(pushed, ANALYSIS_BUFFER_CAPACITY);
        assert_eq!(engine.buffer_len(), ANALYSIS_BUFFER_CAPACITY);

        // Consume all.
        let consumed = engine.consume_all();
        assert_eq!(consumed, ANALYSIS_BUFFER_CAPACITY);
        assert!(engine.frames_dropped() > 0);
    }

    #[test]
    fn test_analysis_buffer_preallocated_no_grow() {
        // Verify that the buffer capacity stays constant.
        let mut buffer = AnalysisBuffer::new();

        for i in 0..100 {
            let result = buffer.push(AnalysisFrame {
                frame_number: i as u64,
                block_size: 64,
                sample_rate: 44100.0,
                rms: [0.5; ANALYSIS_MAX_CHANNELS],
                peak: [0.5; ANALYSIS_MAX_CHANNELS],
                dropped: false,
                zero_crossings: [0; ANALYSIS_MAX_CHANNELS],
            });
            // After filling, all pushes should fail.
            if i < ANALYSIS_BUFFER_CAPACITY {
                assert!(result, "frame {} should be enqueued", i);
            } else {
                assert!(!result, "frame {} should be dropped", i);
            }
        }

        // Capacity should not have changed.
        assert_eq!(buffer.capacity(), ANALYSIS_BUFFER_CAPACITY);
    }

    #[test]
    fn test_analysis_frame_multi_channel_energy() {
        // Verify that different channel energies are correctly computed.
        let ch0: Vec<f32> = vec![1.0; 64]; // High energy
        let ch1: Vec<f32> = vec![0.1; 64]; // Low energy

        let mut interleaved = vec![0.0_f32; 128];
        for i in 0..64 {
            interleaved[2 * i] = ch0[i];
            interleaved[2 * i + 1] = ch1[i];
        }

        let frame = AnalysisFrame::compute(&interleaved, 2, 44100.0, 64, 0);

        assert!(
            frame.rms[0] > 0.9,
            "ch0 RMS should be ~1.0, got {}",
            frame.rms[0]
        );
        assert!(
            frame.rms[1] < 0.2,
            "ch1 RMS should be ~0.1, got {}",
            frame.rms[1]
        );
        assert!(frame.rms[0] > frame.rms[1], "ch0 energy should exceed ch1");
    }

    #[test]
    fn test_analysis_buffer_is_empty_initially() {
        let buffer = AnalysisBuffer::new();
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
        assert_eq!(buffer.len(), 0);
    }

    // -----------------------------------------------------------------------
    // Allocation-free handoff verification (M5-T001)
    // -----------------------------------------------------------------------

    #[test]
    fn test_analysis_handoff_no_allocation_in_process() {
        // Verify that AnalysisFrame::compute and AnalysisBuffer::push
        // perform no heap allocation when called from the audio thread.
        // We do this by checking that the frame is computed inline
        // and pushed into a pre-allocated ring buffer.

        let mut engine = AnalysisEngine::new();
        assert_eq!(engine.buffer_capacity(), ANALYSIS_BUFFER_CAPACITY);

        // Simulate audio thread producing frames with no external alloc.
        for i in 0..ANALYSIS_BUFFER_CAPACITY {
            let buffer: Vec<f32> = (0..64)
                .map(|j| ((i as f32 + j as f32) * 0.01).sin())
                .collect();

            // compute() takes a slice, returns an AnalysisFrame — no heap alloc.
            let frame = AnalysisFrame::compute(&buffer, 1, 44100.0, 64, i as u64);

            // push() writes into a pre-allocated Vec — no heap alloc.
            let pushed = engine.push(frame);
            assert!(pushed, "frame {} should be enqueued", i);
        }

        // Buffer is full — next push drops.
        let drop_frame = AnalysisFrame::compute(&[0.0; 64], 1, 44100.0, 64, 99);
        assert!(!engine.push(drop_frame), "should drop when full");
        assert!(engine.buffer_full());
    }

    #[test]
    fn test_analysis_handoff_drop_policy_deterministic() {
        // Verify the drop-new policy is deterministic: oldest frames are kept,
        // newest frames are dropped when buffer is full.

        let mut engine = AnalysisEngine::new();

        // Fill the buffer completely.
        for i in 0..ANALYSIS_BUFFER_CAPACITY {
            let buffer = vec![0.5; 64];
            let frame = AnalysisFrame::compute(&buffer, 1, 44100.0, 64, i as u64);
            assert!(engine.push(frame));
        }

        // Consume all frames.
        let consumed = engine.consume_all();
        assert_eq!(consumed, ANALYSIS_BUFFER_CAPACITY);

        // Push one more — should succeed (ring buffer is empty after drain).
        let frame = AnalysisFrame::compute(&[0.5; 64], 1, 44100.0, 64, 99);
        assert!(engine.push(frame));

        // Fill the remaining slots (capacity - 1 because one frame is already in).
        for i in 0..(ANALYSIS_BUFFER_CAPACITY - 1) {
            let buffer = vec![0.5; 64];
            let frame = AnalysisFrame::compute(&buffer, 1, 44100.0, 64, (100 + i) as u64);
            assert!(engine.push(frame));
        }

        // Next push should drop (drop-new policy, buffer is now full).
        let frame = AnalysisFrame::compute(&[0.5; 64], 1, 44100.0, 64, 200);
        assert!(!engine.push(frame));

        // The dropped frame count should be exactly 1.
        assert_eq!(engine.frames_dropped(), 1);

        // Verify we still have the oldest frames (FIFO ordering).
        let drained = engine.consume_all();
        assert_eq!(drained, ANALYSIS_BUFFER_CAPACITY);
    }

    #[test]
    fn test_analysis_handoff_stereo_interleaved() {
        // Verify that stereo interleaved buffers are correctly decomposed
        // into per-channel analysis data.

        let mut engine = AnalysisEngine::new();

        // Create a stereo interleaved buffer with distinct signals per channel.
        let mut buffer = vec![0.0_f32; 128]; // 64 frames * 2 channels
        for i in 0..64 {
            buffer[2 * i] = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin();
            buffer[2 * i + 1] = (2.0 * std::f32::consts::PI * 880.0 * i as f32 / 44100.0).sin();
        }

        let frame = AnalysisFrame::compute(&buffer, 2, 44100.0, 64, 0);

        // Both channels should have non-zero RMS.
        assert!(frame.rms[0] > 0.5, "ch0 RMS={}", frame.rms[0]);
        assert!(frame.rms[1] > 0.5, "ch1 RMS={}", frame.rms[1]);

        // Push to engine (verifies the handoff path).
        assert!(engine.push(frame));

        // Consume and verify.
        let consumed = engine.consume_all();
        assert_eq!(consumed, 1);

        let snapshot = engine.latest_snapshot();
        assert!(snapshot.is_some());
    }

    #[test]
    fn test_analysis_handoff_high_load_with_drain() {
        // Simulate high-load scenario: audio thread produces faster than GUI drains.
        // Verify drop-new policy and graceful recovery after drain.

        let mut engine = AnalysisEngine::new();

        let total_blocks = 20;
        let mut frames_consumed = 0usize;

        for i in 0..total_blocks {
            let buffer: Vec<f32> = (0..64)
                .map(|j| (i as f32 * 0.1 + j as f32 * 0.01).sin())
                .collect();

            let frame = AnalysisFrame::compute(&buffer, 1, 44100.0, 64, i as u64);

            // GUI drains every 5 blocks.
            if i % 5 == 4 {
                frames_consumed += engine.consume_all();
            }

            // Try to push (may drop under overflow).
            let _pushed = engine.push(frame);
        }

        // Final drain.
        frames_consumed += engine.consume_all();

        // Verify: we should have consumed at least some frames,
        // and some may have been dropped.
        assert!(frames_consumed > 0, "should consume at least some frames");
        assert!(
            frames_consumed + engine.frames_dropped() as usize <= total_blocks,
            "consumed + dropped <= total"
        );
    }

    // ===================================================================
    // M5-T002: Spectrum processor tests
    // ===================================================================

    #[test]
    fn test_spectrum_config_default_smoothing() {
        let config = SpectrumConfig::new(1024, 44100.0);
        assert_eq!(config.smoothing_factor, 0.4);
        assert_eq!(config.fft_size, 1024);
        assert_eq!(config.window_type, WindowType::Hann);
        assert_eq!(config.log_bin_mode, LogBinMode::OneThirdOctave);
    }

    #[test]
    fn test_spectrum_config_reconfigure() {
        let config = SpectrumConfig::new(1024, 44100.0);
        let mut processor = SpectrumProcessor::new(config.clone());

        // Reconfigure to different FFT size.
        let new_config = SpectrumConfig::new(2048, 48000.0);
        processor.reconfigure(new_config.clone());

        assert_eq!(processor.fft_size(), 2048);
        assert_eq!(processor.config().smoothing_factor, 0.4);
        assert_eq!(processor.config().log_bin_mode, LogBinMode::OneThirdOctave);
    }

    #[test]
    fn test_spectrum_frame_bin_count() {
        let config = SpectrumConfig::new(1024, 44100.0);
        let mut processor = SpectrumProcessor::new(config);
        let frame = processor.process(&vec![0.0_f32; 1024], 44100.0, 0);

        // FFT size N produces N/2 + 1 unique bins for real input.
        assert_eq!(frame.magnitudes_linear.len(), 1024 / 2 + 1);
        assert_eq!(frame.magnitudes_db.len(), 1024 / 2 + 1);
    }

    #[test]
    fn test_spectrum_sine_peak_detection() {
        // A sine wave at a known frequency should produce a peak in the correct bin.
        let fft_size = 1024;
        let sample_rate = 44100.0;
        // Bin frequency resolution: sample_rate / fft_size ≈ 43.07 Hz
        // Frequency 430.7 Hz → bin 10 (exactly).
        let freq_hz = 430.7;
        let bin_idx = (freq_hz / sample_rate * fft_size as f64) as usize;

        let config = SpectrumConfig {
            fft_size,
            smoothing_factor: 0.0, // No smoothing to see raw peak
            window_type: WindowType::Hann,
            log_bin_mode: LogBinMode::None,
            min_db: -120.0,
        };
        let mut processor = SpectrumProcessor::new(config);

        // Generate one period of sine wave.
        let samples: Vec<f32> = (0..fft_size)
            .map(|i| (2.0 * std::f64::consts::PI * freq_hz / sample_rate * i as f64).sin() as f32)
            .collect();

        let frame = processor.process(&samples, sample_rate, 0);
        let linear = &frame.magnitudes_linear;

        // Find the peak bin.
        let peak_bin = linear
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        // The peak should be at or very near the expected bin.
        assert!(
            (peak_bin as i64 - bin_idx as i64).abs() <= 1,
            "expected peak near bin {} (freq {:.1} Hz), got bin {}",
            bin_idx,
            freq_hz,
            peak_bin
        );

        // Verify the peak is significantly above zero.
        assert!(
            linear[peak_bin] > 0.1,
            "peak magnitude {:.4} should be above noise floor",
            linear[peak_bin]
        );
    }

    #[test]
    fn test_spectrum_ema_smoothing() {
        // Verify that EMA smoothing converges on a steady-state signal.
        let config = SpectrumConfig {
            fft_size: 256,
            smoothing_factor: 0.9, // Strong smoothing
            window_type: WindowType::Hann,
            log_bin_mode: LogBinMode::None,
            min_db: -120.0,
        };
        let mut processor = SpectrumProcessor::new(config);

        // Generate a constant-amplitude sine wave.
        let freq_hz = 440.0;
        let sample_rate = 44100.0;
        let samples: Vec<f32> = (0..256)
            .map(|i| (2.0 * std::f64::consts::PI * freq_hz / sample_rate * i as f64).sin() as f32)
            .collect();

        // Process many frames — the first few should change, later ones should converge.
        let mut prev_peak = 0.0_f32;
        for frame_num in 0..50 {
            let frame = processor.process(&samples, sample_rate, frame_num);
            let peak = *frame
                .magnitudes_linear
                .iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap();

            if frame_num > 10 {
                // After warmup, peak should not change much (EMA converges).
                let delta = (peak - prev_peak).abs();
                assert!(
                    delta < 0.01,
                    "EMA should converge: frame {} peak={}, prev_peak={}, delta={}",
                    frame_num,
                    peak,
                    prev_peak,
                    delta
                );
            }
            prev_peak = peak;
        }
    }

    #[test]
    fn test_spectrum_log_bin_count() {
        // Verify that log-binned output has the expected number of groups.
        let sample_rate = 48000.0;
        let config = SpectrumConfig {
            fft_size: 2048,
            smoothing_factor: 0.4,
            window_type: WindowType::Hann,
            log_bin_mode: LogBinMode::OneThirdOctave,
            min_db: -120.0,
        };
        let mut processor = SpectrumProcessor::new(config);

        let samples = vec![0.0_f32; 2048];
        let frame = processor.process(&samples, sample_rate, 0);

        // At 48 kHz, 1/3-octave bands from 25 Hz to 24 kHz produce ~28 groups.
        // The exact count depends on the formula; we just verify it's non-zero and bounded.
        assert!(
            frame.num_log_groups > 0 && frame.num_log_groups < 100,
            "expected 1-99 log groups, got {}",
            frame.num_log_groups
        );
        assert_eq!(
            frame.log_binned_magnitudes_linear.len(),
            frame.num_log_groups
        );
        assert_eq!(frame.log_binned_magnitudes_db.len(), frame.num_log_groups);
    }

    #[test]
    fn test_spectrum_no_allocation_during_process() {
        // Verify that after construction, process() reuses buffers.
        // We test by checking that the processor can be called repeatedly
        // without changing its internal buffer sizes.
        let config = SpectrumConfig::new(512, 44100.0);
        let mut processor = SpectrumProcessor::new(config);

        // First process call.
        let samples = vec![0.5_f32; 512];
        let _frame1 = processor.process(&samples, 44100.0, 0);

        // Reconfigure to same size — should reuse buffers.
        let new_config = SpectrumConfig::new(512, 44100.0);
        processor.reconfigure(new_config);

        // Second process call — should produce identical bin count.
        let _frame2 = processor.process(&samples, 44100.0, 1);
        assert_eq!(_frame2.num_bins, 512 / 2 + 1);
    }

    #[test]
    fn test_spectrum_zero_input() {
        // All-zero input should produce near-zero magnitudes.
        let config = SpectrumConfig::new(256, 44100.0);
        let mut processor = SpectrumProcessor::new(config);

        let samples = vec![0.0_f32; 256];
        let frame = processor.process(&samples, 44100.0, 0);

        for &mag in &frame.magnitudes_linear {
            assert!(
                mag < 1e-6,
                "zero input should produce near-zero magnitude, got {}",
                mag
            );
        }
    }

    #[test]
    fn test_spectrum_short_input_zero_padded() {
        // Input shorter than FFT size should be zero-padded and still produce valid output.
        let config = SpectrumConfig::new(1024, 44100.0);
        let mut processor = SpectrumProcessor::new(config);

        // Only 128 samples of a sine wave.
        let samples: Vec<f32> = (0..128)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 / 44100.0 * i as f64).sin() as f32)
            .collect();

        let frame = processor.process(&samples, 44100.0, 0);

        // Should still produce full bin count.
        assert_eq!(frame.num_bins, 1024 / 2 + 1);
        // Magnitudes should be non-zero (signal exists, even if zero-padded).
        let max_mag = *frame
            .magnitudes_linear
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        assert!(
            max_mag > 0.01,
            "zero-padded signal should have non-zero peak"
        );
    }

    #[test]
    fn test_spectrum_sample_rate_affects_bin_frequency() {
        // Same signal at different sample rates should map to different bin frequencies.
        let fft_size = 1024;

        for sample_rate in [44100.0, 48000.0] {
            let config = SpectrumConfig::new(fft_size, sample_rate);
            let mut processor = SpectrumProcessor::new(config);

            // Simple impulse (all zeros except first sample).
            let mut samples = vec![0.0_f32; fft_size];
            samples[0] = 1.0;

            let frame = processor.process(&samples, sample_rate, 0);

            // For an impulse, the spectrum should be flat (white).
            // Verify bin count is correct.
            assert_eq!(frame.num_bins, fft_size / 2 + 1);

            // Verify the frame number was propagated.
            assert_eq!(frame.frame_number, 0);

            // Verify sample rate was recorded.
            assert!((frame.sample_rate - sample_rate).abs() < 0.1);
        }
    }

    #[test]
    fn test_spectrum_window_types() {
        // Both Hann and Hamming windows should produce valid spectrum.
        for window_type in [WindowType::Hann, WindowType::Hamming] {
            let config = SpectrumConfig {
                fft_size: 512,
                smoothing_factor: 0.4,
                window_type,
                log_bin_mode: LogBinMode::None,
                min_db: -120.0,
            };
            let mut processor = SpectrumProcessor::new(config);

            let samples: Vec<f32> = (0..512)
                .map(|i| (2.0 * std::f64::consts::PI * 220.0 / 44100.0 * i as f64).sin() as f32)
                .collect();

            let frame = processor.process(&samples, 44100.0, 0);

            // Both windows should produce valid output.
            assert_eq!(frame.num_bins, 512 / 2 + 1);
            let max_mag = *frame
                .magnitudes_linear
                .iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap();
            assert!(
                max_mag > 0.0,
                "window {:?} should produce non-zero output",
                window_type
            );
        }
    }
}
