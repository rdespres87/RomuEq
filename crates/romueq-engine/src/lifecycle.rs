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
    // Helper: assert that a closure panics with a specific message
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    fn assert_panics<F: FnOnce()>(f: F) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        assert!(result.is_err(), "expected panic but closure succeeded");
    }
}
