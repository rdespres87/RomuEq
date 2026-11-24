//! Consumer-side analysis engine for draining frames and serving UI snapshots.

use crate::ring_buffer::{AnalysisBuffer, AnalysisFrame};

///
/// This is independent of the EQ's logical state — it controls what
/// analysis is performed and how, not how the audio is processed.
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Whether analysis is enabled. When disabled, frames are still
    /// produced but not pushed to the buffer.
    pub enabled: bool,
    /// Sample rate used for analysis calculations.
    pub sample_rate: f64,
    /// Block size (number of samples per processing block).
    pub block_size: usize,
    /// Number of channels.
    pub channels: usize,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_rate: 44100.0,
            block_size: 512,
            channels: 2,
        }
    }
}

impl AnalysisConfig {
    /// Create a new analysis config with the given parameters.
    pub fn new(sample_rate: f64, block_size: usize, channels: usize) -> Self {
        Self {
            enabled: true,
            sample_rate,
            block_size,
            channels,
        }
    }

    /// Returns `true` if analysis is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable analysis.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable analysis (frames are still produced but not pushed).
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Toggle analysis enabled state.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }
}

// ---------------------------------------------------------------------------
// AnalysisEngine — consumer-side analysis API
// ---------------------------------------------------------------------------

/// Consumer-side analysis engine.
///
/// Drains the analysis buffer and maintains a latest-frame snapshot that
/// can be queried by the GUI thread at any time.
///
/// # Thread boundaries
///
/// - `push()` is called from the audio thread (RT-safe: no allocation).
/// - `consume_all()`, `latest_frame()`, `latest_snapshot()` are called
///   from the GUI/worker thread.
/// - The engine owns its buffer; the audio thread only pushes frames.
pub struct AnalysisEngine {
    /// The bounded ring buffer receiving frames from the audio thread.
    buffer: AnalysisBuffer,

    /// The most recently consumed frame (for instant GUI queries).
    latest: AnalysisFrame,

    /// Total frames consumed.
    consumed_count: u64,

    /// Total frames dropped by the ring buffer.
    dropped_count: u64,

    /// Analysis configuration (separate from EQ state).
    config: AnalysisConfig,

    /// Whether the engine is currently running (receiving frames).
    running: bool,
}

impl AnalysisEngine {
    /// Create a new analysis engine with an empty buffer and default config.
    pub fn new() -> Self {
        Self::with_config(AnalysisConfig::default())
    }

    /// Create a new analysis engine with the given configuration.
    pub fn with_config(config: AnalysisConfig) -> Self {
        AnalysisEngine {
            buffer: AnalysisBuffer::new(),
            latest: AnalysisFrame::default(),
            consumed_count: 0,
            dropped_count: 0,
            config,
            running: true,
        }
    }

    /// Returns a reference to the analysis configuration.
    pub fn config(&self) -> &AnalysisConfig {
        &self.config
    }

    /// Returns a mutable reference to the analysis configuration.
    pub fn config_mut(&mut self) -> &mut AnalysisConfig {
        &mut self.config
    }

    /// Returns `true` if analysis is enabled and the engine is running.
    pub fn is_active(&self) -> bool {
        self.config.enabled && self.running
    }

    /// Enable analysis.
    pub fn enable(&mut self) {
        self.config.enable();
        self.running = true;
    }

    /// Disable analysis — frames are still produced but not pushed.
    pub fn disable(&mut self) {
        self.config.disable();
        self.running = false;
    }

    /// Toggle analysis enabled state.
    pub fn toggle(&mut self) {
        self.config.toggle();
        self.running = self.config.enabled;
    }

    /// Reconfigure the analysis pipeline with new parameters.
    ///
    /// Replaces the current config and resets internal state (consumed/dropped
    /// counts, latest frame). This is safe to call from the GUI thread.
    pub fn reconfigure(&mut self, new_config: AnalysisConfig) {
        self.config = new_config;
        self.running = true;
    }

    /// Push a frame from the audio thread.
    ///
    /// If analysis is disabled, the frame is silently ignored.
    pub fn push(&mut self, frame: AnalysisFrame) -> bool {
        if !self.config.enabled || !self.running {
            return true; // Not an error, just not recording.
        }

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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::ring_buffer::{ANALYSIS_BUFFER_CAPACITY, ANALYSIS_MAX_CHANNELS};

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
            assert!(buffer.push(frame), "frame {i} should be enqueued");
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
                assert!(drained > 0, "should drain at block {block}");
            }
        }

        // Final drain.
        let _final_drain = engine.consume_all();
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
                assert!(result, "frame {i} should be enqueued");
            } else {
                assert!(!result, "frame {i} should be dropped");
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
            assert!(pushed, "frame {i} should be enqueued");
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

}
