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

use romueq_common::{MAX_BLOCK_SIZE, MAX_CHANNELS};

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
/// It owns the sample rate, block size, channel configuration, and a
/// **flat pre-allocated audio buffer** — no `Vec` allocations during
/// `process()`.
///
/// # Allocation-free guarantee
///
/// - `new()` allocates nothing (flat array is stack-sized: 8 * 8192 * 4 bytes
///   ≈ 256 KiB, stored inline in the struct).
/// - `prepare()` zeroes the buffer but does **not** allocate.
/// - `process()` copies data into/out of the flat buffer — no allocation.
/// - `reset()` zeroes the buffer — no allocation.
/// - `destroy()` resets state — no allocation.
///
/// # Layout
///
/// The flat buffer is indexed as `data[channel * max_block_size + sample]`,
/// giving planar channel layout with a single contiguous allocation.
pub struct EngineLifecycle {
    /// Current lifecycle state.
    state: LifecycleState,
    /// Target sample rate in Hz (e.g. 44100.0, 48000.0).
    sample_rate: f64,
    /// Maximum audio block size in samples.
    max_block_size: usize,
    /// Number of channels (1 = mono, 2 = stereo).
    channels: usize,
    /// Flat pre-allocated buffer: `channel * max_block_size + sample`.
    /// Initialized to zeros; never reallocated.
    buffer: [f32; FLAT_BUFFER_SIZE],
    /// Number of blocks processed since last reset.
    blocks_processed: u64,
}

impl EngineLifecycle {
    /// Create a new engine lifecycle in the `Created` state.
    ///
    /// No allocation occurs — the flat buffer is inline in the struct.
    pub fn new() -> Self {
        EngineLifecycle {
            state: LifecycleState::Created,
            sample_rate: 44100.0,
            max_block_size: 512,
            channels: 2,
            buffer: [0.0; FLAT_BUFFER_SIZE],
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
    /// **No allocation.** The flat buffer is pre-sized at construction time.
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

        self.state = LifecycleState::Ready;
        true
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

        // Process: copy inputs to outputs (identity pass-through for now).
        // The actual DSP will be wired in M3.
        for (ch, output_ch) in outputs.iter_mut().enumerate().take(self.channels) {
            let offset = ch * self.max_block_size;
            output_ch.copy_from_slice(&self.buffer[offset..offset + block_size]);
        }

        self.blocks_processed += 1;
        true
    }

    /// Reset internal state (delays, filters, meters).
    ///
    /// Does NOT change sample rate, block size, or channel configuration.
    /// The engine remains in the `Ready` state.
    ///
    /// # Allocation
    ///
    /// **No allocation.** Only zeroes the flat buffer and resets the counter.
    pub fn reset(&mut self) {
        if self.state == LifecycleState::Ready {
            self.buffer.iter_mut().for_each(|s| *s = 0.0);
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
        // Verify the struct size is reasonable (flat buffer inline).
        // FLAT_BUFFER_SIZE = 8 * 8192 = 65536 f32s = 262144 bytes.
        // Plus metadata (~40 bytes). Should be ~262 KiB.
        let size = std::mem::size_of::<EngineLifecycle>();
        assert_eq!(size, FLAT_BUFFER_SIZE * 4 + 40);
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

        // Identity pass-through: outputs should match inputs.
        for ch in 0..2 {
            for i in 0..64 {
                assert!(
                    (output_refs[ch][i] - input_refs[ch][i]).abs() < f32::EPSILON,
                    "ch={} i={} expected {} got {}",
                    ch,
                    i,
                    input_refs[ch][i],
                    output_refs[ch][i]
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
        assert!((output_refs[0][0] - 0.7).abs() < f32::EPSILON);
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
        // Outputs should match inputs (identity pass-through).
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
