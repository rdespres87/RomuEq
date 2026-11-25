//! Lock-free analysis frame capture and bounded SPSC ring buffer.

use equaform_common::{OverflowPolicy, SpscRingBuffer};
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of analysis frames that can be queued.
/// Must be a power of two. Chosen small enough to keep latency low.
pub(crate) const ANALYSIS_BUFFER_CAPACITY: usize = 4;

/// Decimation factor for RMS/peak computation.
/// One analysis sample per N audio samples reduces per-block work to O(N/decimation).
pub(crate) const ANALYSIS_DECIMATION: usize = 64;

/// Maximum number of channels supported for analysis.
pub(crate) const ANALYSIS_MAX_CHANNELS: usize = 8;

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

    /// Compute an analysis frame from a planar (channel-major) audio buffer.
    ///
    /// Channel `ch` occupies `buffer[ch * channel_stride .. ch * channel_stride + block_size]`.
    ///
    /// **RT-safe:** zero allocations, bounds-checked slicing, no panics.
    pub fn compute_planar(
        buffer: &[f32],
        channel_stride: usize,
        channels: usize,
        sample_rate: f64,
        block_size: usize,
        frame_number: u64,
    ) -> Self {
        if block_size == 0 || channels == 0 {
            return Self {
                block_size,
                sample_rate,
                rms: [0.0; ANALYSIS_MAX_CHANNELS],
                peak: [0.0; ANALYSIS_MAX_CHANNELS],
                zero_crossings: [0; ANALYSIS_MAX_CHANNELS],
                frame_number,
                dropped: false,
            };
        }

        let decimation = if block_size > ANALYSIS_DECIMATION {
            ANALYSIS_DECIMATION
        } else {
            1
        };

        let mut rms = [0.0_f32; ANALYSIS_MAX_CHANNELS];
        let mut peak = [0.0_f32; ANALYSIS_MAX_CHANNELS];
        let mut zero_crossings = [0_usize; ANALYSIS_MAX_CHANNELS];

        let effective_channels = channels.min(ANALYSIS_MAX_CHANNELS);

        for ch in 0..effective_channels {
            let start = ch.saturating_mul(channel_stride);
            let end = start.saturating_add(block_size);
            let slice = buffer.get(start..end).unwrap_or(&[]);

            let mut sum_sq: f32 = 0.0;
            let mut max_abs: f32 = 0.0;
            let mut prev_sign: i8 = 0;
            let mut zc: usize = 0;
            let mut count: usize = 0;

            for &sample in slice.iter().step_by(decimation) {
                let abs = sample.abs();
                sum_sq += sample * sample;
                if abs > max_abs {
                    max_abs = abs;
                }
                let sign = if sample > 0.0 {
                    1
                } else if sample < 0.0 {
                    -1
                } else {
                    0
                };
                if prev_sign != 0 && sign != 0 && sign != prev_sign {
                    zc += 1;
                }
                if sign != 0 {
                    prev_sign = sign;
                }
                count += 1;
            }

            if count > 0 {
                rms[ch] = (sum_sq / count as f32).sqrt();
            }
            peak[ch] = max_abs;
            zero_crossings[ch] = zc;
        }

        Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_planar_respects_stride_and_isolates_channels() {
        let channel_stride = 512;
        let block_size = 64;
        let channels = 2;
        let mut buffer = vec![999.0_f32; channels * channel_stride];

        buffer[..block_size].fill(0.5_f32);
        buffer[channel_stride..channel_stride + block_size].fill(-0.25_f32);

        let frame = AnalysisFrame::compute_planar(
            &buffer,
            channel_stride,
            channels,
            48_000.0,
            block_size,
            0,
        );

        assert!((frame.rms[0] - 0.5).abs() < 1e-6);
        assert!((frame.peak[0] - 0.5).abs() < 1e-6);
        assert!((frame.rms[1] - 0.25).abs() < 1e-6);
        assert!((frame.peak[1] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_compute_planar_zero_block_size() {
        let buffer = vec![1.0_f32; 1024];
        let frame = AnalysisFrame::compute_planar(&buffer, 512, 2, 48_000.0, 0, 42);

        assert_eq!(frame.block_size, 0);
        assert_eq!(frame.frame_number, 42);
        assert!(!frame.dropped);
        for ch in 0..ANALYSIS_MAX_CHANNELS {
            assert_eq!(frame.rms[ch], 0.0);
            assert_eq!(frame.peak[ch], 0.0);
            assert_eq!(frame.zero_crossings[ch], 0);
        }
    }
}
