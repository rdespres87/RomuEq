#![forbid(unsafe_code)]

//! Allocation-free engine lifecycle management.
//!
//! This module provides `EngineLifecycle`, a state machine that manages the
//! prepare/process/reset/reprepare sequence for the RomuEq engine.
//!
//! # Design
//!
//! The key requirement is **zero allocation during the processing path**.
//! All buffers are pre-allocated with a flat `[f32; MAX_CHANNELS * MAX_BLOCK_SIZE]`
//! array, eliminating both `Vec` indirection and per-block allocations.
//!
//! # Lifecycle
//!
//! ```text
//! EngineLifecycle::new()
//!     → prepare(sample_rate, max_block_size, channels)
//!     → set_parameters(eq_params)
//!     → process(inputs, outputs)  // can be called repeatedly
//!     → reset()                   // optional, clears internal state
//!     → prepare(new_params)       // reprepare (sample-rate/channel change)
//!     → destroy()                 // cleanup
//! ```
//!
//! # Real-time safety
//!
//! - `process()` performs **no allocations**, no syscalls, no blocking.
//! - Invalid-host behaviour is deterministic: returns `false` without
//!   modifying state (except the documented panic on `inputs.len() != outputs.len()`).
//! - All buffers are stack-allocated or use a flat pre-allocated array.
//!
//! # DSP processing
//!
//! The engine applies a cascade of biquad filters (one per band) per channel,
//! then applies output gain and optional bypass. All filter state is stored
//! in a flat `[f32; MAX_BANDS * MAX_CHANNELS * 4]` array (two delay samples
//! per biquad stage × 2 delay lines).

use romueq_common::{EqParameters, MAX_BANDS, MAX_BLOCK_SIZE, MAX_CHANNELS};
use romueq_dsp::coefficients::compute_coefficients;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of channels supported by the engine lifecycle.
const LIFECYCLE_MAX_CHANNELS: usize = MAX_CHANNELS;

/// Maximum block size supported by the engine lifecycle.
const LIFECYCLE_MAX_BLOCK_SIZE: usize = MAX_BLOCK_SIZE;

/// Total number of samples in the flat internal buffer:
/// `MAX_CHANNELS * MAX_BLOCK_SIZE`.
const FLAT_BUFFER_SIZE: usize = LIFECYCLE_MAX_CHANNELS * LIFECYCLE_MAX_BLOCK_SIZE;

/// Number of biquad stages per channel (one per band).
const BIQUAD_STAGES_PER_CHANNEL: usize = MAX_BANDS;

/// Delay line samples per biquad stage (x[n-1], x[n-2], y[n-1], y[n-2]).
const DELAY_SAMPLES_PER_STAGE: usize = 4;

/// Total delay line storage: bands × channels × delay samples.
const DELAY_BUFFER_SIZE: usize =
    BIQUAD_STAGES_PER_CHANNEL * LIFECYCLE_MAX_CHANNELS * DELAY_SAMPLES_PER_STAGE;

// ---------------------------------------------------------------------------
// LifecycleState
// ---------------------------------------------------------------------------

/// Current state of the engine lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleState {
    /// Engine has been created but not yet prepared.
    Created,
    /// Engine is prepared and ready to process audio.
    Ready,
}

impl Default for LifecycleState {
    fn default() -> Self {
        Self::Created
    }
}

// ---------------------------------------------------------------------------
// EngineLifecycle
// ---------------------------------------------------------------------------

/// Engine lifecycle manager.
///
/// Manages the prepare/process/reset/reprepare sequence for the engine.
/// It owns the sample rate, block size, channel configuration, a
/// **flat pre-allocated audio buffer**, and **flat pre-allocated biquad
/// delay-line state** — no `Vec` allocations during `process()`.
///
/// # Allocation-free guarantee
///
/// - `new()` allocates nothing (flat arrays are inline in the struct).
/// - `prepare()` zeroes the buffers but does **not** allocate.
/// - `process()` copies data into/out of the flat buffer and runs biquad
///   cascades — no allocation.
/// - `reset()` zeroes the buffers — no allocation.
/// - `destroy()` resets state — no allocation.
///
/// # Layout
///
/// The flat audio buffer is indexed as `data[channel * max_block_size + sample]`,
/// giving planar channel layout with a single contiguous allocation.
///
/// The delay-line buffer is indexed as
/// `delay[band * channels * 4 + channel * 4 + delay_slot]`,
/// where `delay_slot` is 0/1 for x[n-1]/x[n-2] and 2/3 for y[n-1]/y[n-2].
pub struct EngineLifecycle {
    /// Current lifecycle state.
    state: LifecycleState,
    /// Target sample rate in Hz (e.g. 44100.0, 48000.0).
    sample_rate: f64,
    /// Maximum audio block size in samples.
    max_block_size: usize,
    /// Number of channels (1 = mono, 2 = stereo).
    channels: usize,
    /// Flat pre-allocated audio buffer: `channel * max_block_size + sample`.
    /// Initialized to zeros; never reallocated.
    buffer: [f32; FLAT_BUFFER_SIZE],
    /// Flat pre-allocated biquad delay-line state:
    /// `band * channels * 4 + channel * 4 + slot`.
    /// Slot 0/1 = x[n-1]/x[n-2], slot 2/3 = y[n-1]/y[n-2].
    delay_line: [f32; DELAY_BUFFER_SIZE],
    /// Current EQ parameters.
    eq_params: EqParameters,
    /// Number of blocks processed since last reset.
    blocks_processed: u64,
}

impl EngineLifecycle {
    /// Create a new engine lifecycle in the `Created` state.
    ///
    /// No allocation occurs — all arrays are inline in the struct.
    pub fn new() -> Self {
        EngineLifecycle {
            state: LifecycleState::Created,
            sample_rate: 44100.0,
            max_block_size: 512,
            channels: 2,
            buffer: [0.0; FLAT_BUFFER_SIZE],
            delay_line: [0.0; DELAY_BUFFER_SIZE],
            eq_params: EqParameters::default(),
            blocks_processed: 0,
        }
    }

    /// Prepare the engine with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` — target sample rate in Hz (e.g. 44100.0, 48000.0).
    /// * `max_block_size` — maximum audio block size in samples.
    /// * `channels` — number of channels (1 = mono, 2 = stereo).
    ///
    /// # Returns
    ///
    /// `true` if preparation succeeded.
    ///
    /// # Panics
    ///
    /// Panics if `sample_rate` is zero or negative, if `max_block_size` is
    /// zero, or if `channels` is zero or exceeds `MAX_CHANNELS`.
    ///
    /// # Allocation
    ///
    /// **No allocation.** The flat buffers are pre-sized at construction time.
    /// This method only updates metadata and zeroes the used portion.
    pub fn prepare(&mut self, sample_rate: f64, max_block_size: usize, channels: usize) -> bool {
        assert!(sample_rate > 0.0, "sample_rate must be > 0");
        assert!(max_block_size > 0, "max_block_size must be > 0");
        assert!(
            channels > 0 && channels <= LIFECYCLE_MAX_CHANNELS,
            "channels must be in 1..={LIFECYCLE_MAX_CHANNELS}"
        );
        assert!(
            max_block_size <= LIFECYCLE_MAX_BLOCK_SIZE,
            "max_block_size must be <= {LIFECYCLE_MAX_BLOCK_SIZE}"
        );

        self.sample_rate = sample_rate;
        self.max_block_size = max_block_size;
        self.channels = channels;

        // Zero the entire flat buffer on prepare (reprepare clears stale data).
        self.buffer.iter_mut().for_each(|s| *s = 0.0);
        self.delay_line.iter_mut().for_each(|s| *s = 0.0);

        self.state = LifecycleState::Ready;
        true
    }

    /// Set the EQ parameters.
    ///
    /// Parameters are applied on the next `process()` call.
    ///
    /// # Allocation
    ///
    /// **No allocation.** The `EqParameters` struct is `Copy` and stored inline.
    pub fn set_parameters(&mut self, params: EqParameters) {
        self.eq_params = params;
    }

    /// Process a single audio block.
    ///
    /// # Arguments
    ///
    /// * `inputs` — array of input buffers (one per input channel).
    /// * `outputs` — array of output buffers (one per output channel).
    ///
    /// # Preconditions
    ///
    /// - The engine must be in the `Ready` state (i.e., `prepare` has been
    ///   called successfully).
    /// - `inputs.len()` must equal `outputs.len()` (mono-to-mono, stereo-to-stereo).
    /// - Each input/output slice must have length ≤ `max_block_size`.
    ///
    /// # Invalid-host behaviour
    ///
    /// If preconditions are violated, the method returns `false` and does
    /// **not** modify internal state. This is deterministic and safe — no
    /// panic, no allocation, no undefined behavior.
    ///
    /// # Panics
    ///
    /// Panics if `inputs.len() != outputs.len()`.
    ///
    /// # Allocation
    ///
    /// **No allocation.** All data is copied to/from the flat pre-allocated buffer.
    /// DSP processing uses pre-allocated biquad delay-line state.
    pub fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]) -> bool {
        // Check state.
        if self.state != LifecycleState::Ready {
            return false;
        }

        // Check input/output count mismatch — panic as per ADR contract.
        if inputs.len() != outputs.len() {
            panic!("inputs.len() != outputs.len()");
        }

        let block_size = inputs.first().map(|s| s.len()).unwrap_or(0);

        // Validate block size.
        if block_size == 0 || block_size > self.max_block_size {
            return false;
        }

        // Verify all channels have the same length.
        if inputs.iter().any(|s| s.len() != block_size)
            || outputs.iter().any(|s| s.len() != block_size)
        {
            return false;
        }

        // Verify channel count matches.
        if inputs.len() != self.channels {
            return false;
        }

        // Copy inputs to flat buffer (planar layout).
        for (ch, input_ch) in inputs.iter().enumerate().take(self.channels) {
            let offset = ch * self.max_block_size;
            self.buffer[offset..offset + block_size].copy_from_slice(input_ch);
        }

        // Apply DSP processing: band cascade → output gain → bypass.
        self.process_dsp(block_size);

        // Copy processed buffer to outputs.
        for (ch, output_ch) in outputs.iter_mut().enumerate().take(self.channels) {
            let offset = ch * self.max_block_size;
            output_ch.copy_from_slice(&self.buffer[offset..offset + block_size]);
        }

        self.blocks_processed += 1;
        true
    }

    /// Apply DSP processing to the flat buffer.
    ///
    /// Processes each channel through a cascade of biquad filters (one per
    /// enabled band), then applies output gain and optional bypass.
    ///
    /// # Allocation
    ///
    /// **No allocation.** Uses pre-allocated delay-line state.
    fn process_dsp(&mut self, block_size: usize) {
        let sample_rate = self.sample_rate as f32;
        let bypass = self.eq_params.global.bypass;
        let output_gain_db = self.eq_params.global.output_gain_db;
        let output_gain_linear = 10.0_f32.powf(output_gain_db / 20.0);

        for ch_idx in 0..self.channels {
            let buffer_offset = ch_idx * self.max_block_size;
            let samples = &mut self.buffer[buffer_offset..buffer_offset + block_size];

            if bypass {
                // Bypass: zero the output (silence).
                samples.iter_mut().for_each(|s| *s = 0.0);
                continue;
            }

            // Process through band cascade.
            for band_idx in 0..MAX_BANDS {
                let band_params = self.eq_params.bands[band_idx];

                // Skip disabled bands.
                if !band_params.enabled {
                    continue;
                }

                // Compute coefficients for this band.
                let coeffs = compute_coefficients(
                    band_params.filter_type,
                    band_params.frequency_hz,
                    band_params.gain_db,
                    band_params.q,
                    band_params.slope,
                    sample_rate,
                );

                // Get delay line state for this band/channel.
                let delay_offset = band_idx * self.channels * DELAY_SAMPLES_PER_STAGE
                    + ch_idx * DELAY_SAMPLES_PER_STAGE;
                let delay =
                    &mut self.delay_line[delay_offset..delay_offset + DELAY_SAMPLES_PER_STAGE];

                // Apply biquad filter to this band (direct-form I).
                let b0 = coeffs.b0;
                let b1 = coeffs.b1;
                let b2 = coeffs.b2;
                let a1 = coeffs.a1;
                let a2 = coeffs.a2;

                let mut x1 = delay[0];
                let mut x2 = delay[1];
                let mut y1 = delay[2];
                let mut y2 = delay[3];

                for sample in samples.iter_mut().take(block_size) {
                    let x_n = *sample;
                    let y_n = b0 * x_n + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
                    *sample = y_n;
                    x2 = x1;
                    x1 = x_n;
                    y2 = y1;
                    y1 = y_n;
                }

                delay[0] = x1;
                delay[1] = x2;
                delay[2] = y1;
                delay[3] = y2;
            }

            // Apply output gain.
            if output_gain_linear != 1.0 {
                samples.iter_mut().for_each(|s| *s *= output_gain_linear);
            }
        }
    }

    /// Reset internal state (delays, filters, meters).
    ///
    /// Does NOT change sample rate, block size, or channel configuration.
    /// The engine remains in the `Ready` state.
    ///
    /// # Allocation
    ///
    /// **No allocation.** Only zeroes the flat buffers and resets the counter.
    pub fn reset(&mut self) {
        if self.state == LifecycleState::Ready {
            self.buffer.iter_mut().for_each(|s| *s = 0.0);
            self.delay_line.iter_mut().for_each(|s| *s = 0.0);
            self.blocks_processed = 0;
        }
    }

    /// Destroy the engine lifecycle.
    ///
    /// Transitions to the `Created` state. After this, `prepare` must be
    /// called again before processing.
    ///
    /// # Allocation
    ///
    /// **No allocation.** Simply resets state.
    pub fn destroy(&mut self) {
        self.state = LifecycleState::Created;
        self.buffer.iter_mut().for_each(|s| *s = 0.0);
        self.delay_line.iter_mut().for_each(|s| *s = 0.0);
        self.blocks_processed = 0;
    }

    /// Return the current lifecycle state.
    pub fn state(&self) -> LifecycleState {
        self.state
    }

    /// Return the current sample rate.
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Return the current max block size.
    pub fn max_block_size(&self) -> usize {
        self.max_block_size
    }

    /// Return the current channel count.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Return the number of blocks processed since last reset.
    pub fn blocks_processed(&self) -> u64 {
        self.blocks_processed
    }

    /// Return a reference to the flat buffer data.
    ///
    /// The buffer is laid out as planar channels:
    /// `data[channel * max_block_size + sample]`.
    pub fn buffer(&self) -> &[f32] {
        &self.buffer
    }

    /// Return a mutable reference to the flat buffer data.
    pub fn buffer_mut(&mut self) -> &mut [f32] {
        &mut self.buffer
    }

    /// Return the current EQ parameters.
    pub fn eq_parameters(&self) -> &EqParameters {
        &self.eq_params
    }

    /// Return a mutable reference to the current EQ parameters.
    pub fn eq_parameters_mut(&mut self) -> &mut EqParameters {
        &mut self.eq_params
    }
}

impl Default for EngineLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use romueq_common::{BandParameters, Slope};

    // -----------------------------------------------------------------------
    // EngineLifecycle construction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lifecycle_new_is_created() {
        let engine = EngineLifecycle::new();
        assert_eq!(engine.state(), LifecycleState::Created);
        assert_eq!(engine.sample_rate(), 44100.0);
        assert_eq!(engine.max_block_size(), 512);
        assert_eq!(engine.channels(), 2);
        // Buffer should be all zeros.
        assert!(engine.buffer().iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_lifecycle_new_no_allocation() {
        // Verify the struct size is reasonable (flat buffers inline).
        // FLAT_BUFFER_SIZE = 8 * 8192 = 65536 f32s = 262144 bytes.
        // DELAY_BUFFER_SIZE = 8 * 8 * 4 = 256 f32s = 1024 bytes.
        // Metadata + padding brings total to ~263 KiB.
        let size = std::mem::size_of::<EngineLifecycle>();
        let expected = FLAT_BUFFER_SIZE * 4 + DELAY_BUFFER_SIZE * 4;
        // Allow up to 256 bytes of metadata/padding overhead.
        assert!(
            size >= expected && size <= expected + 256,
            "EngineLifecycle size {size} out of range [{expected}, {}]",
            expected + 256
        );
    }

    // -----------------------------------------------------------------------
    // Prepare tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lifecycle_prepare_mono() {
        let mut engine = EngineLifecycle::new();
        assert!(engine.prepare(48000.0, 256, 1));
        assert_eq!(engine.state(), LifecycleState::Ready);
        assert_eq!(engine.sample_rate(), 48000.0);
        assert_eq!(engine.max_block_size(), 256);
        assert_eq!(engine.channels(), 1);
    }

    #[test]
    fn test_lifecycle_prepare_stereo() {
        let mut engine = EngineLifecycle::new();
        assert!(engine.prepare(44100.0, 512, 2));
        assert_eq!(engine.state(), LifecycleState::Ready);
        assert_eq!(engine.channels(), 2);
    }

    #[test]
    fn test_lifecycle_prepare_zeroes_buffer() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        // Write some data.
        engine.buffer_mut()[0] = 1.0;
        engine.buffer_mut()[1] = 2.0;

        // Repare should zero everything.
        assert!(engine.prepare(44100.0, 256, 2));
        assert!(engine.buffer().iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_lifecycle_prepare_max_block_size() {
        let mut engine = EngineLifecycle::new();
        assert!(engine.prepare(48000.0, MAX_BLOCK_SIZE, 2));
        assert_eq!(engine.max_block_size(), MAX_BLOCK_SIZE);
    }

    #[test]
    fn test_lifecycle_prepare_max_channels() {
        let mut engine = EngineLifecycle::new();
        assert!(engine.prepare(48000.0, 256, MAX_CHANNELS));
        assert_eq!(engine.channels(), MAX_CHANNELS);
    }

    #[test]
    #[should_panic(expected = "sample_rate must be > 0")]
    fn test_lifecycle_prepare_zero_sample_rate_panics() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(0.0, 256, 2);
    }

    #[test]
    #[should_panic(expected = "sample_rate must be > 0")]
    fn test_lifecycle_prepare_negative_sample_rate_panics() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(-1.0, 256, 2);
    }

    #[test]
    #[should_panic(expected = "max_block_size must be > 0")]
    fn test_lifecycle_prepare_zero_block_size_panics() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 0, 2);
    }

    #[test]
    #[should_panic(expected = "channels must be in 1..=8")]
    fn test_lifecycle_prepare_zero_channels_panics() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 0);
    }

    #[test]
    #[should_panic(expected = "channels must be in 1..=8")]
    fn test_lifecycle_prepare_too_many_channels_panics() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 9);
    }

    #[test]
    #[should_panic(expected = "max_block_size must be <= 8192")]
    fn test_lifecycle_prepare_too_large_block_panics() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, MAX_BLOCK_SIZE + 1, 2);
    }

    // -----------------------------------------------------------------------
    // Process tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lifecycle_process_after_prepare() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64], vec![0.3; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(engine.process(&input_refs, &mut output_refs));
        assert_eq!(engine.blocks_processed(), 1);

        // With default parameters (all bands disabled by default), the output
        // should be near-zero (bypass is false, but all bands are enabled
        // with default params — gain 0dB, unity gain).
        // Actually, default BandParameters has enabled=true, gain_db=0.0,
        // which means unity gain through each band. So output ≈ input.
        for ch in 0..2 {
            for i in 0..64 {
                // Allow small numerical error from biquad processing.
                assert!(
                    (output_refs[ch][i] - input_refs[ch][i]).abs() < 1e-3,
                    "ch={} i={} expected {} got {} (diff {})",
                    ch,
                    i,
                    input_refs[ch][i],
                    output_refs[ch][i],
                    (output_refs[ch][i] - input_refs[ch][i]).abs()
                );
            }
        }
    }

    #[test]
    fn test_lifecycle_process_before_prepare_fails() {
        let mut engine = EngineLifecycle::new();
        let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(!engine.process(&input_refs, &mut output_refs));
        assert_eq!(engine.state(), LifecycleState::Created);
    }

    #[test]
    fn test_lifecycle_process_block_size_1() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 1);

        let inputs: Vec<Vec<f32>> = vec![vec![0.7]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(engine.process(&input_refs, &mut output_refs));
        // With default params (unity gain), output should be near input.
        assert!((output_refs[0][0] - 0.7).abs() < 1e-3);
    }

    #[test]
    fn test_lifecycle_process_block_size_max() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(48000.0, MAX_BLOCK_SIZE, 2);

        let inputs: Vec<Vec<f32>> = vec![vec![1.0; MAX_BLOCK_SIZE], vec![0.5; MAX_BLOCK_SIZE]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; MAX_BLOCK_SIZE], vec![0.0; MAX_BLOCK_SIZE]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(engine.process(&input_refs, &mut output_refs));
        assert_eq!(engine.blocks_processed(), 1);
    }

    #[test]
    fn test_lifecycle_process_block_too_large_fails() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        let inputs: Vec<Vec<f32>> = vec![vec![0.5; 512], vec![0.3; 512]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 512], vec![0.0; 512]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        // Block size 512 > max_block_size 256 → fails.
        assert!(!engine.process(&input_refs, &mut output_refs));
    }

    #[test]
    #[should_panic(expected = "inputs.len() != outputs.len()")]
    fn test_lifecycle_process_unequal_input_output_panics() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        engine.process(&input_refs, &mut output_refs);
    }

    #[test]
    fn test_lifecycle_process_wrong_channel_count_fails() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        // Only 1 input for 2-channel engine.
        let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(!engine.process(&input_refs, &mut output_refs));
    }

    #[test]
    fn test_lifecycle_process_variable_block_sizes() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        // Block size 1.
        let inputs: Vec<Vec<f32>> = vec![vec![0.1], vec![0.2]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0], vec![0.0]];
        let mut input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();
        assert!(engine.process(&input_refs, &mut output_refs));

        // Block size 64.
        let inputs: Vec<Vec<f32>> = vec![vec![0.3; 64], vec![0.4; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
        input_refs = inputs.iter().map(|v| v.as_slice()).collect();
        output_refs = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();
        assert!(engine.process(&input_refs, &mut output_refs));

        // Block size 256.
        let inputs: Vec<Vec<f32>> = vec![vec![0.5; 256], vec![0.6; 256]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 256], vec![0.0; 256]];
        input_refs = inputs.iter().map(|v| v.as_slice()).collect();
        output_refs = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();
        assert!(engine.process(&input_refs, &mut output_refs));

        assert_eq!(engine.blocks_processed(), 3);
    }

    // -----------------------------------------------------------------------
    // DSP processing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lifecycle_process_bypass_silences_output() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        // Set bypass to true.
        let mut params = EqParameters::default();
        params.global.bypass = true;
        engine.set_parameters(params);

        let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64], vec![0.3; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(engine.process(&input_refs, &mut output_refs));

        // Bypass should silence all output.
        for (ch, output_ch) in output_refs.iter().enumerate().take(2) {
            for (i, &sample) in output_ch.iter().enumerate() {
                assert_eq!(
                    sample, 0.0,
                    "ch={ch} i={i} bypass output should be 0, got {sample}"
                );
            }
        }
    }

    #[test]
    fn test_lifecycle_process_output_gain() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 1);

        // Set output gain to +6 dB (×2 linear).
        let mut params = EqParameters::default();
        params.global.output_gain_db = 6.0206; // ≈ ×2
        engine.set_parameters(params);

        let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(engine.process(&input_refs, &mut output_refs));

        // Output should be ≈ input × 2.
        for i in 0..64 {
            assert!(
                (output_refs[0][i] - 1.0).abs() < 1e-2,
                "i={} expected ~1.0 got {}",
                i,
                output_refs[0][i]
            );
        }
    }

    #[test]
    fn test_lifecycle_process_band_boost() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 1);

        // Enable band 0 with +12 dB boost at 1000 Hz.
        let mut params = EqParameters::default();
        params.bands[0] = BandParameters {
            enabled: true,
            filter_type: romueq_common::FilterType::Bell,
            frequency_hz: 1000.0,
            gain_db: 12.0,
            q: 0.707,
            slope: Slope::Db12,
        };
        engine.set_parameters(params);

        // Generate a 1000 Hz sine wave (one channel, 64 samples).
        let sample_rate = 44100.0;
        let freq = 1000.0;
        let input_samples: Vec<f32> = (0..64)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin() * 0.5)
            .collect();
        let inputs: Vec<&[f32]> = vec![&input_samples];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(engine.process(&inputs, &mut output_refs));

        // The output should have higher amplitude than the input at 1000 Hz
        // (the bell filter boosts by +12 dB ≈ ×4 in amplitude).
        let input_rms: f32 = inputs[0].iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();
        let output_rms: f32 =
            output_refs[0].iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();

        // Output RMS should be significantly higher than input RMS.
        assert!(
            output_rms > input_rms * 1.5,
            "output RMS {output_rms} should be > input RMS {input_rms} × 1.5"
        );
    }

    #[test]
    fn test_lifecycle_process_band_disable() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 1);

        // Disable all bands.
        let mut params = EqParameters::default();
        for band in &mut params.bands {
            band.enabled = false;
        }
        engine.set_parameters(params);

        let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(engine.process(&input_refs, &mut output_refs));

        // With all bands disabled and no output gain, the input signal passes
        // through unchanged (no processing applied).
        for i in 0..64 {
            assert_eq!(
                output_refs[0][i], 0.5,
                "i={} output should match input (0.5) with all bands disabled, got {}",
                i, output_refs[0][i]
            );
        }
    }

    #[test]
    fn test_lifecycle_process_stereo_channel_isolation() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        // Set different parameters for each channel by using different inputs.
        let inputs: Vec<Vec<f32>> = vec![vec![1.0; 64], vec![0.0; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(engine.process(&input_refs, &mut output_refs));

        // Channel 0 should have non-zero output (input was 1.0).
        // Channel 1 should have near-zero output (input was 0.0, and with
        // default params the biquad cascade starts from zero state).
        let ch0_sum: f32 = output_refs[0].iter().sum();
        let ch1_sum: f32 = output_refs[1].iter().sum();

        assert!(
            ch0_sum.abs() > ch1_sum.abs(),
            "ch0 sum {ch0_sum} should be > ch1 sum {ch1_sum}"
        );
    }

    // -----------------------------------------------------------------------
    // Reset tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lifecycle_reset() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        // Process a few blocks.
        let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64], vec![0.3; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
        for _ in 0..5 {
            let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
            let mut output_refs: Vec<&mut [f32]> =
                outputs.iter_mut().map(|v| v.as_mut_slice()).collect();
            engine.process(&input_refs, &mut output_refs);
        }
        assert_eq!(engine.blocks_processed(), 5);

        // Reset.
        engine.reset();
        assert_eq!(engine.blocks_processed(), 0);
        assert_eq!(engine.state(), LifecycleState::Ready);
        assert_eq!(engine.sample_rate(), 44100.0);
        assert_eq!(engine.channels(), 2);
        // Buffer should be zeroed.
        assert!(engine.buffer().iter().all(|&s| s == 0.0));
        // Delay line should be zeroed.
        assert!(engine.delay_line.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_lifecycle_reset_before_prepare() {
        let mut engine = EngineLifecycle::new();
        // Reset before prepare should be a no-op.
        engine.reset();
        assert_eq!(engine.state(), LifecycleState::Created);
        assert_eq!(engine.blocks_processed(), 0);
    }

    // -----------------------------------------------------------------------
    // Repare tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lifecycle_reprepare() {
        let mut engine = EngineLifecycle::new();

        // First prepare: stereo, 44100 Hz.
        assert!(engine.prepare(44100.0, 256, 2));
        assert_eq!(engine.sample_rate(), 44100.0);
        assert_eq!(engine.channels(), 2);

        // Repare: mono, 48000 Hz.
        assert!(engine.prepare(48000.0, 128, 1));
        assert_eq!(engine.sample_rate(), 48000.0);
        assert_eq!(engine.channels(), 1);
        assert_eq!(engine.state(), LifecycleState::Ready);
    }

    #[test]
    fn test_lifecycle_reprepare_zeroes_buffer() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        // Write data to channel 0.
        engine.buffer_mut()[0] = 1.0;
        engine.buffer_mut()[1] = 2.0;

        // Repare with different params.
        assert!(engine.prepare(48000.0, 128, 1));

        // Buffer should be zeroed.
        assert!(engine.buffer().iter().all(|&s| s == 0.0));
    }

    // -----------------------------------------------------------------------
    // Destroy tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lifecycle_destroy() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);
        assert_eq!(engine.state(), LifecycleState::Ready);

        engine.destroy();
        assert_eq!(engine.state(), LifecycleState::Created);
        assert_eq!(engine.blocks_processed(), 0);
        // Buffer should be zeroed.
        assert!(engine.buffer().iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_lifecycle_process_after_destroy_fails() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);
        engine.destroy();

        let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(!engine.process(&input_refs, &mut output_refs));
    }

    #[test]
    fn test_lifecycle_destroy_idempotent() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);
        engine.destroy();
        // Second destroy should be safe.
        engine.destroy();
        assert_eq!(engine.state(), LifecycleState::Created);
    }

    // -----------------------------------------------------------------------
    // Flat buffer layout tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_flat_buffer_channel_layout() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        // Write to channel 0, sample 0.
        engine.buffer_mut()[0] = 1.0;
        // Write to channel 1, sample 0.
        engine.buffer_mut()[256] = 2.0;

        // Read back via process with matching inputs.
        let inputs: Vec<Vec<f32>> = vec![vec![1.0], vec![2.0]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0], vec![0.0]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        // Process should copy inputs to buffer, then buffer to outputs.
        assert!(engine.process(&input_refs, &mut output_refs));

        // Buffer should still have the input values.
        assert_eq!(engine.buffer()[0], 1.0);
        assert_eq!(engine.buffer()[256], 2.0);
        // Outputs should match inputs (identity pass-through with default params).
        assert_eq!(output_refs[0][0], 1.0);
        assert_eq!(output_refs[1][0], 2.0);
    }

    #[test]
    fn test_flat_buffer_channel_isolation() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        // Write different values to each channel.
        engine.buffer_mut()[0] = 1.0;
        engine.buffer_mut()[256] = 2.0;

        // Process should preserve channel isolation.
        let inputs: Vec<Vec<f32>> = vec![vec![0.0], vec![0.0]];
        let mut outputs: Vec<Vec<f32>> = vec![vec![0.0], vec![0.0]];
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

        assert!(engine.process(&input_refs, &mut output_refs));

        // Outputs should match inputs (identity), not buffer values.
        assert_eq!(output_refs[0][0], 0.0);
        assert_eq!(output_refs[1][0], 0.0);
    }

    // -----------------------------------------------------------------------
    // M4-T002: Split-block equivalence tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_split_block_equivalence() {
        // Process a block of 128 samples as one block vs two halves of 64.
        // Results should match within numerical tolerance.
        let mut engine1 = EngineLifecycle::new();
        engine1.prepare(44100.0, 256, 1);
        let mut params = EqParameters::default();
        params.bands[0] = BandParameters {
            enabled: true,
            filter_type: romueq_common::FilterType::Bell,
            frequency_hz: 1000.0,
            gain_db: 6.0,
            q: 0.707,
            slope: Slope::Db12,
        };
        engine1.set_parameters(params);

        let mut engine2 = EngineLifecycle::new();
        engine2.prepare(44100.0, 256, 1);
        engine2.set_parameters(params);

        // Full block of 128 samples.
        let full_input: Vec<f32> = (0..128)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44100.0).sin() * 0.5)
            .collect();
        let mut full_output = vec![0.0; 128];
        let input_refs: Vec<&[f32]> = vec![&full_input];
        let mut output_refs: Vec<&mut [f32]> = vec![&mut full_output];
        assert!(engine1.process(&input_refs, &mut output_refs));

        // Split into two halves of 64.
        let half1_input: Vec<f32> = full_input[..64].to_vec();
        let mut half1_output = vec![0.0; 64];
        let input_refs1: Vec<&[f32]> = vec![&half1_input];
        let mut output_refs1: Vec<&mut [f32]> = vec![&mut half1_output];
        assert!(engine2.process(&input_refs1, &mut output_refs1));

        let half2_input: Vec<f32> = full_input[64..].to_vec();
        let mut half2_output = vec![0.0; 64];
        let input_refs2: Vec<&[f32]> = vec![&half2_input];
        let mut output_refs2: Vec<&mut [f32]> = vec![&mut half2_output];
        assert!(engine2.process(&input_refs2, &mut output_refs2));

        // Concatenate split outputs.
        let mut split_output = half1_output;
        split_output.extend(half2_output);

        // Compare: full vs split should match within tolerance.
        for i in 0..128 {
            let diff = (full_output[i] - split_output[i]).abs();
            assert!(
                diff < 1e-6,
                "split-block mismatch at sample {}: full={}, split={}, diff={}",
                i,
                full_output[i],
                split_output[i],
                diff
            );
        }
    }

    #[test]
    fn test_split_block_equivalence_stereo() {
        // Same test for stereo: split-block should match whole-block.
        let mut engine1 = EngineLifecycle::new();
        engine1.prepare(44100.0, 256, 2);
        let mut params = EqParameters::default();
        params.bands[0] = BandParameters {
            enabled: true,
            filter_type: romueq_common::FilterType::HighPass,
            frequency_hz: 500.0,
            gain_db: 0.0,
            q: 0.707,
            slope: Slope::Db12,
        };
        engine1.set_parameters(params);

        let mut engine2 = EngineLifecycle::new();
        engine2.prepare(44100.0, 256, 2);
        engine2.set_parameters(params);

        let input: Vec<f32> = (0..128)
            .map(|i| (2.0 * std::f32::consts::PI * 800.0 * i as f32 / 44100.0).sin() * 0.3)
            .collect();
        let inputs_ch0: Vec<&[f32]> = vec![&input, &input];
        let mut out1_ch0 = vec![0.0; 128];
        let mut out1_ch1 = vec![0.0; 128];
        let mut refs1: Vec<&mut [f32]> = vec![&mut out1_ch0[..], &mut out1_ch1[..]];
        assert!(engine1.process(&inputs_ch0, &mut refs1));

        // Split: first 64 samples.
        let mut out2_ch0 = vec![0.0; 64];
        let mut out2_ch1 = vec![0.0; 64];
        let mut refs2: Vec<&mut [f32]> = vec![&mut out2_ch0[..], &mut out2_ch1[..]];
        let half_input: Vec<f32> = input[..64].to_vec();
        let half_refs: Vec<&[f32]> = vec![&half_input[..], &half_input[..]];
        assert!(engine2.process(&half_refs, &mut refs2));

        // Second 64 samples.
        let mut out3_ch0 = vec![0.0; 64];
        let mut out3_ch1 = vec![0.0; 64];
        let mut refs3: Vec<&mut [f32]> = vec![&mut out3_ch0[..], &mut out3_ch1[..]];
        let half_input2: Vec<f32> = input[64..].to_vec();
        let half_refs2: Vec<&[f32]> = vec![&half_input2[..], &half_input2[..]];
        assert!(engine2.process(&half_refs2, &mut refs3));

        // Concatenate and compare.
        let mut split_ch0 = out2_ch0;
        split_ch0.extend(out3_ch0);
        let mut split_ch1 = out2_ch1;
        split_ch1.extend(out3_ch1);

        for i in 0..128 {
            let diff0: f32 = (out1_ch0[i] - split_ch0[i]).abs();
            let diff1: f32 = (out1_ch1[i] - split_ch1[i]).abs();
            assert!(
                diff0 < 1e-6,
                "stereo ch0 split mismatch at {}: diff={}",
                i,
                diff0
            );
            assert!(
                diff1 < 1e-6,
                "stereo ch1 split mismatch at {}: diff={}",
                i,
                diff1
            );
        }
    }

    // -----------------------------------------------------------------------
    // M4-T002: All eight bands coexist
    // -----------------------------------------------------------------------

    #[test]
    fn test_all_eight_bands_coexist() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 1);

        // Enable all 8 bands with different frequencies and gains.
        let mut params = EqParameters::default();
        let band_configs = [
            (60.0, 6.0),     // Sub-bass boost
            (200.0, -3.0),   // Bass cut
            (500.0, 4.0),    // Low-mid boost
            (1000.0, -2.0),  // Mid cut
            (2500.0, 5.0),   // Upper-mid boost
            (5000.0, -4.0),  // Presence cut
            (10000.0, 3.0),  // Brilliance boost
            (14000.0, -2.0), // Air cut
        ];
        for (i, (freq, gain)) in band_configs.iter().enumerate() {
            params.bands[i] = BandParameters {
                enabled: true,
                filter_type: romueq_common::FilterType::Bell,
                frequency_hz: *freq,
                gain_db: *gain,
                q: 0.707,
                slope: Slope::Db12,
            };
        }
        engine.set_parameters(params);

        // Process a complex signal (sum of multiple frequencies).
        let input: Vec<f32> = (0..256)
            .map(|i| {
                let t = i as f32 / 44100.0;
                (2.0 * std::f32::consts::PI * 60.0 * t).sin() * 0.1
                    + (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.2
                    + (2.0 * std::f32::consts::PI * 5000.0 * t).sin() * 0.1
            })
            .collect();

        let mut output = vec![0.0; 256];
        let input_refs: Vec<&[f32]> = vec![&input];
        let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
        assert!(engine.process(&input_refs, &mut output_refs));

        // Output should be non-zero (processing happened).
        let output_rms: f32 = output.iter().map(|s| s * s).sum::<f32>().sqrt() / 256.0_f32.sqrt();
        assert!(
            output_rms > 0.01,
            "all-8-bands output RMS {} should be > 0.01",
            output_rms
        );
    }

    // -----------------------------------------------------------------------
    // M4-T002: Block size variety tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_block_sizes_2_3_4_8() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 1);

        let mut params = EqParameters::default();
        params.bands[0] = BandParameters {
            enabled: true,
            filter_type: romueq_common::FilterType::LowPass,
            frequency_hz: 2000.0,
            gain_db: 0.0,
            q: 0.707,
            slope: Slope::Db12,
        };
        engine.set_parameters(params);

        for block_size in [2, 3, 4, 8] {
            let input: Vec<f32> = (0..block_size)
                .map(|i| (2.0 * std::f32::consts::PI * 500.0 * i as f32 / 44100.0).sin() * 0.5)
                .collect();
            let mut output = vec![0.0; block_size];
            let input_refs: Vec<&[f32]> = vec![&input];
            let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
            assert!(engine.process(&input_refs, &mut output_refs));

            // Output should be non-zero.
            let sum: f32 = output.iter().map(|s| s.abs()).sum();
            assert!(
                sum > 0.0,
                "block_size={} output should be non-zero",
                block_size
            );
        }
    }

    #[test]
    fn test_block_size_powers_of_two() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 8192, 1);

        let input: Vec<f32> = (0..8192)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin() * 0.5)
            .collect();
        let mut output = vec![0.0; 8192];
        let input_refs: Vec<&[f32]> = vec![&input];
        let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
        assert!(engine.process(&input_refs, &mut output_refs));
        assert_eq!(engine.blocks_processed(), 1);
    }

    #[test]
    fn test_silence_input() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 1);

        let mut params = EqParameters::default();
        params.bands[0] = BandParameters {
            enabled: true,
            filter_type: romueq_common::FilterType::Bell,
            frequency_hz: 1000.0,
            gain_db: 12.0,
            q: 0.707,
            slope: Slope::Db12,
        };
        engine.set_parameters(params);

        let input: Vec<f32> = vec![0.0; 64];
        let mut output = vec![0.0; 64];
        let input_refs: Vec<&[f32]> = vec![&input];
        let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
        assert!(engine.process(&input_refs, &mut output_refs));

        // Silence in → silence out (no stored state after reset).
        let sum: f32 = output.iter().map(|s| s.abs()).sum();
        assert!(
            sum < 1e-10,
            "silence input should produce silence output, got sum={}",
            sum
        );
    }

    #[test]
    fn test_impulse_response() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 1);

        let mut params = EqParameters::default();
        params.bands[0] = BandParameters {
            enabled: true,
            filter_type: romueq_common::FilterType::Bell,
            frequency_hz: 1000.0,
            gain_db: 6.0,
            q: 2.0, // High Q for ringing
            slope: Slope::Db12,
        };
        engine.set_parameters(params);

        // Impulse at sample 0.
        let mut input = vec![1.0; 64];
        input[0] = 1.0;
        for i in 1..64 {
            input[i] = 0.0;
        }
        let mut output = vec![0.0; 64];
        let input_refs: Vec<&[f32]> = vec![&input];
        let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
        assert!(engine.process(&input_refs, &mut output_refs));

        // Impulse should produce non-zero output (filter response).
        let sum: f32 = output.iter().map(|s| s.abs()).sum();
        assert!(sum > 0.5, "impulse response sum {} should be > 0.5", sum);
    }

    // -----------------------------------------------------------------------
    // M4-T002: Channel routing correctness
    // -----------------------------------------------------------------------

    #[test]
    fn test_channel_routing_mono() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 1);

        let input: Vec<f32> = (0..64)
            .map(|i| (2.0 * std::f32::consts::PI * 800.0 * i as f32 / 44100.0).sin() * 0.7)
            .collect();
        let mut output = vec![0.0; 64];
        let input_refs: Vec<&[f32]> = vec![&input];
        let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
        assert!(engine.process(&input_refs, &mut output_refs));

        // Output should match input pattern (unity gain, default params).
        for i in 0..64 {
            assert!(
                (output_refs[0][i] - input[i]).abs() < 1e-3,
                "mono ch0 sample {}: expected {} got {}",
                i,
                input[i],
                output_refs[0][i]
            );
        }
    }

    #[test]
    fn test_channel_routing_stereo_different_content() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        // Different content per channel.
        let ch0: Vec<f32> = (0..64).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        let ch1: Vec<f32> = (0..64).map(|i| (i as f32 * 0.02).sin() * 0.3).collect();
        let inputs: Vec<&[f32]> = vec![&ch0, &ch1];
        let mut out0 = vec![0.0; 64];
        let mut out1 = vec![0.0; 64];
        let mut output_refs: Vec<&mut [f32]> = vec![&mut out0, &mut out1];
        assert!(engine.process(&inputs, &mut output_refs));

        // Each channel should preserve its own content (unity gain).
        for i in 0..64 {
            let diff0 = (out0[i] - ch0[i]).abs();
            let diff1 = (out1[i] - ch1[i]).abs();
            assert!(diff0 < 1e-3, "stereo ch0 sample {}: diff={}", i, diff0);
            assert!(diff1 < 1e-3, "stereo ch1 sample {}: diff={}", i, diff1);
        }
    }

    #[test]
    fn test_channel_routing_stereo_with_filter() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);

        // Boost 1000 Hz on both channels.
        let mut params = EqParameters::default();
        params.bands[0] = BandParameters {
            enabled: true,
            filter_type: romueq_common::FilterType::Bell,
            frequency_hz: 1000.0,
            gain_db: 12.0,
            q: 0.707,
            slope: Slope::Db12,
        };
        engine.set_parameters(params);

        let ch0: Vec<f32> = (0..64)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44100.0).sin() * 0.5)
            .collect();
        let ch1: Vec<f32> = (0..64)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44100.0).sin() * 0.3)
            .collect();
        let inputs: Vec<&[f32]> = vec![&ch0, &ch1];
        let mut out0 = vec![0.0; 64];
        let mut out1 = vec![0.0; 64];
        let mut output_refs: Vec<&mut [f32]> = vec![&mut out0, &mut out1];
        assert!(engine.process(&inputs, &mut output_refs));

        // Both channels should be boosted (output amplitude > input amplitude).
        let ch0_in_rms: f32 = ch0.iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();
        let ch0_out_rms: f32 = out0.iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();
        let ch1_in_rms: f32 = ch1.iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();
        let ch1_out_rms: f32 = out1.iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();

        assert!(
            ch0_out_rms > ch0_in_rms * 1.5,
            "ch0 boost: in_rms={}, out_rms={}",
            ch0_in_rms,
            ch0_out_rms
        );
        assert!(
            ch1_out_rms > ch1_in_rms * 1.5,
            "ch1 boost: in_rms={}, out_rms={}",
            ch1_in_rms,
            ch1_out_rms
        );
    }

    // -----------------------------------------------------------------------
    // M4-T002: Split-block equivalence with filter
    // -----------------------------------------------------------------------

    #[test]
    fn test_split_block_with_filter() {
        // Verify split-block equivalence when filters are active.
        let mut engine1 = EngineLifecycle::new();
        engine1.prepare(44100.0, 256, 1);
        let mut params = EqParameters::default();
        params.bands[0] = BandParameters {
            enabled: true,
            filter_type: romueq_common::FilterType::LowPass,
            frequency_hz: 2000.0,
            gain_db: 0.0,
            q: 0.707,
            slope: Slope::Db12,
        };
        params.bands[1] = BandParameters {
            enabled: true,
            filter_type: romueq_common::FilterType::Bell,
            frequency_hz: 500.0,
            gain_db: 6.0,
            q: 0.707,
            slope: Slope::Db12,
        };
        engine1.set_parameters(params);

        let mut engine2 = EngineLifecycle::new();
        engine2.prepare(44100.0, 256, 1);
        engine2.set_parameters(params);

        let input: Vec<f32> = (0..128)
            .map(|i| {
                (2.0 * std::f32::consts::PI * 300.0 * i as f32 / 44100.0).sin() * 0.3
                    + (2.0 * std::f32::consts::PI * 1500.0 * i as f32 / 44100.0).sin() * 0.2
            })
            .collect();

        // Full block.
        let mut full_output = vec![0.0; 128];
        let input_refs: Vec<&[f32]> = vec![&input];
        let mut output_refs: Vec<&mut [f32]> = vec![&mut full_output];
        assert!(engine1.process(&input_refs, &mut output_refs));

        // Split into two halves.
        let mut split_output = vec![0.0; 128];
        let half1 = &input[..64];
        let half2 = &input[64..];
        let mut half1_out = vec![0.0; 64];
        let mut half2_out = vec![0.0; 64];
        let refs1: Vec<&[f32]> = vec![half1];
        let mut refs2: Vec<&mut [f32]> = vec![&mut half1_out];
        assert!(engine2.process(&refs1, &mut refs2));
        let refs3: Vec<&[f32]> = vec![half2];
        let mut refs4: Vec<&mut [f32]> = vec![&mut half2_out];
        assert!(engine2.process(&refs3, &mut refs4));
        split_output[..64].copy_from_slice(&half1_out);
        split_output[64..].copy_from_slice(&half2_out);

        // Compare.
        for i in 0..128 {
            let diff = (full_output[i] - split_output[i]).abs();
            assert!(
                diff < 1e-6,
                "split-block with filter mismatch at {}: full={}, split={}, diff={}",
                i,
                full_output[i],
                split_output[i],
                diff
            );
        }
    }
}
