#![forbid(unsafe_code)]

//! Audio buffer representation and lifecycle management for RomuEq.
//!
//! This module defines the planar f32 audio buffer type used throughout the
//! engine and plugin boundary. It also provides the `EngineLifecycle` type
//! that manages the prepare/process/reset/reprepare sequence.
//!
//! # Buffer representation
//!
//! Audio data is stored in **planar** format: one contiguous `f32` slice per
//! channel. For a stereo buffer of 256 samples, the memory layout is:
//!
//! ```text
//! channel 0: [f32; 256]  // samples 0..256
//! channel 1: [f32; 256]  // samples 0..256
//! ```
//!
//! This matches the CLAP and VST3 conventions and avoids interleaved copies.
//!
//! # Lifecycle contract
//!
//! The engine follows a strict state machine:
//!
//! ```text
//! Created → prepare(sample_rate, max_block_size, channels) → Ready
//! Ready → process(inputs, outputs) → Ready
//! Ready → reset() → Ready
//! Ready → prepare(new_params) → Ready (reprepare)
//! ```
//!
//! - `process` is RT-safe: no allocation, no blocking, no syscalls.
//! - `reset` clears internal state (delays, filters, meters) but does not
//!   change sample rate or channel configuration.
//! - `prepare` reallocates internal buffers if needed and is called outside
//!   the RT path (e.g. from the host callback).

// ---------------------------------------------------------------------------
// AudioBuffer — planar f32 mono/stereo buffer
// ---------------------------------------------------------------------------

/// Maximum number of channels supported.
pub const MAX_CHANNELS: usize = 8;

/// Maximum block size supported (samples per channel).
pub const MAX_BLOCK_SIZE: usize = 8192;

/// A planar f32 audio buffer.
///
/// Memory layout: one contiguous slice per channel, all of the same length.
/// The buffer is pre-allocated with `prepare()` and reused across blocks
/// to avoid per-block allocation.
#[derive(Debug)]
pub struct AudioBuffer {
    /// Planar data: `data[channel][sample]`.
    data: Vec<Vec<f32>>,
    /// Number of channels (1 = mono, 2 = stereo).
    channels: usize,
    /// Number of samples per channel.
    length: usize,
}

impl AudioBuffer {
    /// Create a new empty buffer (zero channels, zero length).
    pub fn new() -> Self {
        AudioBuffer {
            data: Vec::new(),
            channels: 0,
            length: 0,
        }
    }

    /// Prepare the buffer with the given channel count and sample length.
    ///
    /// # Arguments
    ///
    /// * `channels` — number of channels (1 or 2 for MVP).
    /// * `length` — number of samples per channel.
    ///
    /// # Panics
    ///
    /// Panics if `channels` is zero or exceeds `MAX_CHANNELS`, or if
    /// `length` is zero or exceeds `MAX_BLOCK_SIZE`.
    pub fn prepare(&mut self, channels: usize, length: usize) {
        assert!(
            channels > 0 && channels <= MAX_CHANNELS,
            "channels must be in 1..={MAX_CHANNELS}"
        );
        assert!(
            length > 0 && length <= MAX_BLOCK_SIZE,
            "length must be in 1..={MAX_BLOCK_SIZE}"
        );

        self.channels = channels;
        self.length = length;

        // Reuse existing allocations when possible.
        if self.data.len() == channels {
            for slice in self.data.iter_mut() {
                if slice.len() > length {
                    slice.truncate(length);
                } else if slice.len() < length {
                    slice.resize(length, 0.0);
                }
                // Zero out all samples on reprepare.
                for sample in slice.iter_mut() {
                    *sample = 0.0;
                }
            }
        } else {
            self.data = (0..channels).map(|_| vec![0.0; length]).collect();
        }
    }

    /// Return the number of channels.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Return the number of samples per channel.
    pub fn length(&self) -> usize {
        self.length
    }

    /// Get a reference to the data for channel `ch`.
    ///
    /// # Panics
    ///
    /// Panics if `ch` is out of range.
    pub fn channel(&self, ch: usize) -> &[f32] {
        assert!(ch < self.channels, "channel {ch} out of range");
        &self.data[ch]
    }

    /// Get a mutable reference to the data for channel `ch`.
    ///
    /// # Panics
    ///
    /// Panics if `ch` is out of range.
    pub fn channel_mut(&mut self, ch: usize) -> &mut [f32] {
        assert!(ch < self.channels, "channel {ch} out of range");
        &mut self.data[ch]
    }

    /// Get all input channels as a slice of slices.
    pub fn as_input_slices(&self) -> Vec<&[f32]> {
        (0..self.channels).map(|ch| self.channel(ch)).collect()
    }

    /// Get all output channels as a mutable slice of slices.
    pub fn as_output_slices(&mut self) -> Vec<&mut [f32]> {
        let mut result = Vec::with_capacity(self.channels);
        let mut data = &mut self.data[..];
        for _ in 0..self.channels {
            let (first, rest) = data.split_at_mut(1);
            result.push(first[0].as_mut_slice());
            data = rest;
        }
        result
    }

    /// Fill all channels with zeros.
    pub fn zero(&mut self) {
        for ch in 0..self.channels {
            for sample in self.channel_mut(ch).iter_mut() {
                *sample = 0.0;
            }
        }
    }

    /// Returns `true` if the buffer has been prepared (non-zero channels and length).
    pub fn is_prepared(&self) -> bool {
        self.channels > 0 && self.length > 0
    }
}

impl Default for AudioBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EngineLifecycle — prepare/process/reset/reprepare state machine
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

/// Engine lifecycle manager.
///
/// Manages the prepare/process/reset/reprepare sequence for the engine.
/// It owns the sample rate, block size, channel configuration, and audio
/// buffers. It does not own DSP state — that is managed by individual
/// plugins/bands.
///
/// # Lifecycle
///
/// ```text
/// EngineLifecycle::new()
///     → prepare(sample_rate, max_block_size, channels)
///     → process(inputs, outputs)  // can be called repeatedly
///     → reset()                   // optional, clears internal state
///     → prepare(new_params)       // reprepare (sample-rate/channel change)
///     → destroy()                 // cleanup
/// ```
pub struct EngineLifecycle {
    state: LifecycleState,
    sample_rate: f64,
    max_block_size: usize,
    channels: usize,
    /// Internal audio buffer for processing.
    buffer: AudioBuffer,
    /// Number of blocks processed since last reset.
    blocks_processed: u64,
}

impl EngineLifecycle {
    /// Create a new engine lifecycle in the `Created` state.
    pub fn new() -> Self {
        EngineLifecycle {
            state: LifecycleState::Created,
            sample_rate: 44100.0,
            max_block_size: 512,
            channels: 2,
            buffer: AudioBuffer::new(),
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
    pub fn prepare(&mut self, sample_rate: f64, max_block_size: usize, channels: usize) -> bool {
        assert!(sample_rate > 0.0, "sample_rate must be > 0");
        assert!(max_block_size > 0, "max_block_size must be > 0");
        assert!(
            channels > 0 && channels <= MAX_CHANNELS,
            "channels must be in 1..={MAX_CHANNELS}"
        );

        self.sample_rate = sample_rate;
        self.max_block_size = max_block_size;
        self.channels = channels;

        // Prepare the internal buffer for the maximum block size.
        self.buffer.prepare(channels, max_block_size);

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
    /// # Invalid-host behavior
    ///
    /// If preconditions are violated, the method returns `false` and does
    /// **not** modify internal state. This is deterministic and safe — no
    /// panic, no allocation, no undefined behavior.
    ///
    /// # Panics
    ///
    /// Panics if `inputs.len() != outputs.len()`.
    pub fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]) -> bool {
        // Check preconditions.
        if self.state != LifecycleState::Ready {
            return false;
        }

        if inputs.len() != outputs.len() {
            panic!("inputs.len() != outputs.len()");
        }

        let block_size = inputs.first().map(|s| s.len()).unwrap_or(0);

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

        // Resize internal buffer if the actual block size differs from max.
        if self.buffer.length() != block_size {
            self.buffer.prepare(self.channels, block_size);
        }

        // Copy inputs to internal buffer.
        for (ch, input) in inputs.iter().enumerate() {
            self.buffer.channel_mut(ch).copy_from_slice(input);
        }

        // Process: copy inputs to outputs (identity pass-through for now).
        // The actual DSP will be wired in M3.
        for (ch, output) in outputs.iter_mut().enumerate() {
            output.copy_from_slice(self.buffer.channel(ch));
        }

        self.blocks_processed += 1;
        true
    }

    /// Reset internal state (delays, filters, meters).
    ///
    /// Does NOT change sample rate, block size, or channel configuration.
    /// The engine remains in the `Ready` state.
    pub fn reset(&mut self) {
        if self.state == LifecycleState::Ready {
            self.buffer.zero();
            self.blocks_processed = 0;
        }
    }

    /// Destroy the engine lifecycle.
    ///
    /// Transitions to the `Created` state. After this, `prepare` must be
    /// called again before processing.
    pub fn destroy(&mut self) {
        self.state = LifecycleState::Created;
        self.buffer = AudioBuffer::new();
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

    /// Return a reference to the internal buffer.
    pub fn buffer(&self) -> &AudioBuffer {
        &self.buffer
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
    // AudioBuffer tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_buffer_new_is_empty() {
        let buf = AudioBuffer::new();
        assert_eq!(buf.channels(), 0);
        assert_eq!(buf.length(), 0);
        assert!(!buf.is_prepared());
    }

    #[test]
    fn test_buffer_prepare_mono() {
        let mut buf = AudioBuffer::new();
        buf.prepare(1, 256);
        assert_eq!(buf.channels(), 1);
        assert_eq!(buf.length(), 256);
        assert!(buf.is_prepared());
        assert_eq!(buf.channel(0).len(), 256);
        // All zeros after prepare.
        assert!(buf.channel(0).iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_buffer_prepare_stereo() {
        let mut buf = AudioBuffer::new();
        buf.prepare(2, 512);
        assert_eq!(buf.channels(), 2);
        assert_eq!(buf.length(), 512);
        assert_eq!(buf.channel(0).len(), 512);
        assert_eq!(buf.channel(1).len(), 512);
    }

    #[test]
    fn test_buffer_prepare_single_sample() {
        let mut buf = AudioBuffer::new();
        buf.prepare(1, 1);
        assert_eq!(buf.channels(), 1);
        assert_eq!(buf.length(), 1);
    }

    #[test]
    fn test_buffer_prepare_large_block() {
        let mut buf = AudioBuffer::new();
        buf.prepare(2, MAX_BLOCK_SIZE);
        assert_eq!(buf.channels(), 2);
        assert_eq!(buf.length(), MAX_BLOCK_SIZE);
    }

    #[test]
    #[should_panic(expected = "channels must be in 1..=8")]
    fn test_buffer_prepare_zero_channels_panics() {
        let mut buf = AudioBuffer::new();
        buf.prepare(0, 256);
    }

    #[test]
    #[should_panic(expected = "channels must be in 1..=8")]
    fn test_buffer_prepare_too_many_channels_panics() {
        let mut buf = AudioBuffer::new();
        buf.prepare(9, 256);
    }

    #[test]
    #[should_panic(expected = "length must be in 1..=8192")]
    fn test_buffer_prepare_zero_length_panics() {
        let mut buf = AudioBuffer::new();
        buf.prepare(2, 0);
    }

    #[test]
    #[should_panic(expected = "length must be in 1..=8192")]
    fn test_buffer_prepare_too_large_block_panics() {
        let mut buf = AudioBuffer::new();
        buf.prepare(2, MAX_BLOCK_SIZE + 1);
    }

    #[test]
    fn test_buffer_write_and_read() {
        let mut buf = AudioBuffer::new();
        buf.prepare(2, 4);
        buf.channel_mut(0)[0] = 1.0;
        buf.channel_mut(0)[1] = 2.0;
        buf.channel_mut(1)[0] = 3.0;
        buf.channel_mut(1)[1] = 4.0;

        assert_eq!(buf.channel(0)[0], 1.0);
        assert_eq!(buf.channel(0)[1], 2.0);
        assert_eq!(buf.channel(1)[0], 3.0);
        assert_eq!(buf.channel(1)[1], 4.0);
    }

    #[test]
    fn test_buffer_zero() {
        let mut buf = AudioBuffer::new();
        buf.prepare(2, 4);
        buf.channel_mut(0)[0] = 1.0;
        buf.channel_mut(1)[1] = 2.0;
        buf.zero();
        assert!(buf.channel(0).iter().all(|&s| s == 0.0));
        assert!(buf.channel(1).iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_buffer_reuse_on_reprepare() {
        let mut buf = AudioBuffer::new();
        buf.prepare(2, 256);
        buf.channel_mut(0)[0] = 1.0;

        // Re-prepare with smaller block size — should reuse allocation.
        buf.prepare(2, 128);
        assert_eq!(buf.channels(), 2);
        assert_eq!(buf.length(), 128);
        // Data is zeroed because resize fills with 0.0.
        assert_eq!(buf.channel(0)[0], 0.0);
    }

    #[test]
    fn test_buffer_as_input_output_slices() {
        let mut buf = AudioBuffer::new();
        buf.prepare(2, 4);
        let inputs = buf.as_input_slices();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].len(), 4);

        let outputs = buf.as_output_slices();
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].len(), 4);
    }

    // -----------------------------------------------------------------------
    // EngineLifecycle tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lifecycle_new_is_created() {
        let engine = EngineLifecycle::new();
        assert_eq!(engine.state(), LifecycleState::Created);
        assert_eq!(engine.sample_rate(), 44100.0);
        assert_eq!(engine.max_block_size(), 512);
        assert_eq!(engine.channels(), 2);
    }

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
                assert!((output_refs[ch][i] - input_refs[ch][i]).abs() < f32::EPSILON);
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
    }

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
    fn test_lifecycle_destroy() {
        let mut engine = EngineLifecycle::new();
        engine.prepare(44100.0, 256, 2);
        assert_eq!(engine.state(), LifecycleState::Ready);

        engine.destroy();
        assert_eq!(engine.state(), LifecycleState::Created);
        assert!(!engine.buffer().is_prepared());
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
    fn test_lifecycle_variable_block_sizes() {
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

    #[test]
    fn test_lifecycle_invalid_sample_rate_panics() {
        let mut engine = EngineLifecycle::new();
        assert_panics(|| {
            engine.prepare(0.0, 256, 2);
        });
        assert_panics(|| {
            engine.prepare(-1.0, 256, 2);
        });
    }

    #[test]
    fn test_lifecycle_invalid_max_block_size_panics() {
        let mut engine = EngineLifecycle::new();
        assert_panics(|| {
            engine.prepare(44100.0, 0, 2);
        });
    }

    #[test]
    fn test_lifecycle_invalid_channels_panics() {
        let mut engine = EngineLifecycle::new();
        assert_panics(|| {
            engine.prepare(44100.0, 256, 0);
        });
        assert_panics(|| {
            engine.prepare(44100.0, 256, 9);
        });
    }

    // -----------------------------------------------------------------------
    // Helper: assert that a closure panics with a specific message
    // -----------------------------------------------------------------------

    fn assert_panics<F: FnOnce()>(f: F) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        assert!(result.is_err(), "expected panic but closure succeeded");
    }
}
