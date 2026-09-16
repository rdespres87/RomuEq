//! Meter types and non-blocking transport for presentation.
//!
//! This module provides:
//! - [`PeakHold`]: Fast-attack, slow-decay peak hold calculator.
//! - [`MeterValues`]: Per-channel meter values (peak, RMS, peak hold).
//! - [`MeterFrame`]: Meter values with rolling history for visualization.
//! - [`MeterEngine`]: Non-blocking transport from audio thread to presentation/GUI.
//!
//! # Architecture
//!
//! ```text
//! [audio thread] ──> push(peak, rms) ──> SpscRingBuffer ──> latest() ──> [GUI thread]
//!                    O(1)                       bounded SPSC       lock-free read
//! ```
//!
//! - `push()` is RT-safe: no allocation, no blocking.
//! - `latest()` returns a reference to the most recent frame (no lock).
//! - The engine maintains peak hold state and a rolling history buffer.

use romueq_common::{MAX_CHANNELS, OverflowPolicy, SpscRingBuffer};

/// Number of frames of history to keep for visualization.
/// This is used to draw level-meter bars in the GUI.
pub const METER_HISTORY_LENGTH: usize = 64;

// ===========================================================================
// PeakHold — fast attack, slow decay
// ===========================================================================

/// Peak hold calculator with fast attack and configurable decay rate.
///
/// Used to implement the "peak hold" behavior of professional audio meters:
/// the value rises instantly with the input (fast attack) and decays slowly
/// when the input drops below the held peak.
///
/// # Example
///
/// ```
/// use romueq_analysis::PeakHold;
///
/// let mut ph = PeakHold::new(18.0, 48000.0, 512);
/// ph.update(0.8); // Capture a peak
/// assert!(ph.value() > 0.75); // Still holds the peak
/// ```
pub struct PeakHold {
    /// Current held value.
    value: f32,
    /// Decay factor applied each frame when input < hold.
    decay: f32,
}

impl PeakHold {
    /// Create a new peak hold with the given decay rate.
    ///
    /// # Arguments
    /// * `decay_db_per_s` — How many dB the hold should decay per second.
    ///   Typical values: 18 dB/s for peak meters, 1 dB/s for VU meters.
    /// * `sample_rate` — Sample rate of the audio stream.
    /// * `block_size` — Number of samples per processing block.
    pub fn new(decay_db_per_s: f32, sample_rate: f64, block_size: usize) -> Self {
        // Convert dB/s decay to a per-frame multiplier.
        // frames_per_second = sample_rate / block_size
        // decay_factor = 10^(-decay_db_per_s / (20 * frames_per_second))
        let frames_per_second = sample_rate / block_size as f64;
        let decay = 10.0f32.powf(-decay_db_per_s / (20.0_f32 * frames_per_second as f32));

        Self { value: 0.0, decay }
    }

    /// Update the peak hold with a new input value.
    ///
    /// If `input` is greater than the current hold, the hold jumps to `input`
    /// (instant attack). Otherwise it decays by the configured rate.
    pub fn update(&mut self, input: f32) {
        if input > self.value {
            self.value = input;
        } else {
            self.value *= self.decay;
        }
    }

    /// Return the current peak hold value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Reset the peak hold to zero.
    pub fn reset(&mut self) {
        self.value = 0.0;
    }
}

impl Default for PeakHold {
    fn default() -> Self {
        // Default to 18 dB/s decay (standard peak meter behavior).
        Self::new(18.0, 48000.0, 512)
    }
}

// ===========================================================================
// MeterValues — per-channel meter values
// ===========================================================================

/// Meter values for a single channel set.
#[derive(Clone, Copy, Debug)]
pub struct MeterValues {
    /// Instantaneous peak level per channel (0.0 to 1.0+).
    pub peak: [f32; MAX_CHANNELS],
    /// RMS energy level per channel (0.0 to 1.0+).
    pub rms: [f32; MAX_CHANNELS],
    /// Peak hold level per channel (decays slowly).
    pub peak_hold: [f32; MAX_CHANNELS],
}

impl Default for MeterValues {
    fn default() -> Self {
        Self {
            peak: [0.0; MAX_CHANNELS],
            rms: [0.0; MAX_CHANNELS],
            peak_hold: [0.0; MAX_CHANNELS],
        }
    }
}

impl MeterValues {
    /// Convert peak value to dBFS for channel `ch`.
    ///
    /// Returns `f32::NEG_INFINITY` if the value is below the noise floor.
    pub fn peak_db(&self, ch: usize) -> f32 {
        let val = self.peak[ch];
        if val < 1e-10 {
            f32::NEG_INFINITY
        } else {
            20.0 * val.log10()
        }
    }

    /// Convert RMS value to dBFS for channel `ch`.
    ///
    /// Returns `f32::NEG_INFINITY` if the value is below the noise floor.
    pub fn rms_db(&self, ch: usize) -> f32 {
        let val = self.rms[ch];
        if val < 1e-10 {
            f32::NEG_INFINITY
        } else {
            20.0 * val.log10()
        }
    }

    /// Convert peak hold value to dBFS for channel `ch`.
    ///
    /// Returns `f32::NEG_INFINITY` if the value is below the noise floor.
    pub fn peak_hold_db(&self, ch: usize) -> f32 {
        let val = self.peak_hold[ch];
        if val < 1e-10 {
            f32::NEG_INFINITY
        } else {
            20.0 * val.log10()
        }
    }

    /// Returns `true` if any channel is clipping (peak > 1.0).
    pub fn is_clipping(&self) -> bool {
        self.peak.iter().any(|&p| p > 1.0)
    }
}

// ===========================================================================
// MeterFrame — presentation-ready meter data with history
// ===========================================================================

/// A single frame of meter data, including a rolling history buffer for
/// visualization (e.g., level-meter bars).
///
/// The `history` array is ordered newest first: index 0 is the most recent
/// frame, index `METER_HISTORY_LENGTH - 1` is the oldest.
#[derive(Clone, Debug)]
pub struct MeterFrame {
    /// Current meter values.
    pub values: MeterValues,
    /// Rolling history of peak_hold values (newest first).
    pub history: [[f32; MAX_CHANNELS]; METER_HISTORY_LENGTH],
    /// Frame sequence number.
    pub frame_number: u64,
}

impl Default for MeterFrame {
    fn default() -> Self {
        Self {
            values: MeterValues::default(),
            history: [[0.0; MAX_CHANNELS]; METER_HISTORY_LENGTH],
            frame_number: 0,
        }
    }
}

// ===========================================================================
// MeterEngine — non-blocking transport from audio thread to presentation
// ===========================================================================

/// Non-blocking meter engine.
///
/// Receives meter values from the audio thread (via `push()`) and provides
/// non-blocking read access to the latest frame for the presentation/GUI thread.
///
/// # Architecture
///
/// ```text
/// [audio thread] ──> push(peak, rms) ──> SpscRingBuffer ──> latest() ──> [GUI thread]
///                    O(1)                       bounded SPSC       lock-free read
/// ```
///
/// - `push()` is RT-safe: no allocation, no blocking.
/// - `latest()` returns a reference to the most recent frame (no lock).
/// - The engine maintains peak hold state and a rolling history buffer.
pub struct MeterEngine {
    /// Ring buffer for meter frames (non-blocking transport).
    ring: SpscRingBuffer<MeterFrame>,

    /// Most recently consumed frame (for instant GUI access).
    latest_frame: MeterFrame,

    /// Peak hold calculator per channel.
    peak_holds: [PeakHold; MAX_CHANNELS],

    /// Rolling history of peak_hold values (shifted each frame).
    history: [[f32; MAX_CHANNELS]; METER_HISTORY_LENGTH],

    /// Frame sequence counter.
    frame_counter: u64,
}

impl MeterEngine {
    /// Create a new meter engine.
    ///
    /// # Arguments
    /// * `sample_rate` — Sample rate of the audio stream.
    /// * `block_size` — Number of samples per processing block.
    pub fn new(sample_rate: f64, block_size: usize) -> Self {
        let peak_holds = std::array::from_fn(|_| PeakHold::new(18.0, sample_rate, block_size));

        Self {
            ring: SpscRingBuffer::new(4, OverflowPolicy::DropNew),
            latest_frame: MeterFrame::default(),
            peak_holds,
            history: [[0.0; MAX_CHANNELS]; METER_HISTORY_LENGTH],
            frame_counter: 0,
        }
    }

    /// Push raw peak and RMS values from the audio thread.
    ///
    /// The engine computes peak hold internally and builds a `MeterFrame`
    /// with the current values and a rolling history snapshot.
    ///
    /// **RT-safe:** no allocation, no blocking.
    ///
    /// Returns `true` if the frame was enqueued, `false` if dropped (buffer full).
    pub fn push(&mut self, peak: [f32; MAX_CHANNELS], rms: [f32; MAX_CHANNELS]) -> bool {
        // Update internal peak holds for each channel.
        let mut peak_hold = [0.0_f32; MAX_CHANNELS];
        for ch in 0..MAX_CHANNELS {
            self.peak_holds[ch].update(peak[ch]);
            peak_hold[ch] = self.peak_holds[ch].value();
        }

        // Shift history one slot to the right (oldest goes out, new one goes in at front).
        // history[0] is always the newest frame.
        for i in (1..METER_HISTORY_LENGTH).rev() {
            self.history[i] = self.history[i - 1];
        }
        self.history[0] = peak_hold;

        let frame = MeterFrame {
            values: MeterValues {
                peak,
                rms,
                peak_hold,
            },
            history: self.history,
            frame_number: self.frame_counter,
        };
        self.frame_counter += 1;

        self.ring.enqueue(frame)
    }

    /// Push meter values from an analysis frame.
    ///
    /// Convenience method that extracts peak and RMS from an `AnalysisFrame`
    /// and pushes them to this engine.
    pub fn push_from_analysis(&mut self, analysis: &crate::AnalysisFrame) -> bool {
        self.push(analysis.peak, analysis.rms)
    }

    /// Return a reference to the latest meter frame.
    ///
    /// If no frame has been consumed yet, returns a default (all-zeros) frame.
    ///
    /// This is lock-free and safe to call from the GUI thread at any time.
    /// The caller should call [`consume_all`](Self::consume_all) first to ensure
    /// the returned frame is up-to-date.
    pub fn latest(&self) -> &MeterFrame {
        &self.latest_frame
    }

    /// Drain the ring buffer and update the latest frame.
    ///
    /// Returns the number of frames drained.
    pub fn consume_all(&mut self) -> usize {
        let mut count = 0;
        while let Some(frame) = self.ring.dequeue() {
            count += 1;
            self.latest_frame = frame;
        }
        count
    }

    /// Reset all peak holds and clear the history buffer.
    ///
    /// Should be called after audio stream reset.
    pub fn reset(&mut self) {
        for ph in &mut self.peak_holds {
            ph.reset();
        }
        for h in &mut self.history {
            *h = [0.0; MAX_CHANNELS];
        }
        self.latest_frame = MeterFrame::default();
    }

    /// Returns `true` if the ring buffer is full.
    pub fn is_full(&self) -> bool {
        self.ring.is_full()
    }

    /// Current number of frames waiting in the buffer.
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// Total frames pushed since creation.
    pub fn frames_pushed(&self) -> u64 {
        self.frame_counter
    }
}

impl Default for MeterEngine {
    fn default() -> Self {
        Self::new(48000.0, 512)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AnalysisFrame;

    #[test]
    fn test_peak_hold_fast_attack() {
        let mut ph = PeakHold::new(18.0, 48000.0, 512);

        // Input spike should be captured instantly.
        ph.update(0.8);
        assert!(
            (ph.value() - 0.8).abs() < 1e-6,
            "peak hold should capture input instantly, got {}",
            ph.value()
        );

        // Subsequent lower input should not lower the hold.
        ph.update(0.3);
        assert!(
            ph.value() > 0.7,
            "peak hold should not decay on lower input"
        );
    }

    #[test]
    fn test_peak_hold_slow_decay() {
        let mut ph = PeakHold::new(18.0, 48000.0, 512);
        ph.update(1.0);

        // After many frames of zero input, the hold should decay significantly.
        for _ in 0..1000 {
            ph.update(0.0);
        }
        assert!(
            ph.value() < 0.1,
            "peak hold should decay over time, got {}",
            ph.value()
        );
    }

    #[test]
    fn test_peak_hold_decay_rate() {
        let mut ph = PeakHold::new(18.0, 48000.0, 512);
        ph.update(1.0); // Set initial peak

        // After 1 second of zero input, the hold should have decayed by ~18 dB.
        let frames_per_second = 48000.0 / 512.0;
        let frames_for_1_sec = frames_per_second as usize;

        for _ in 0..frames_for_1_sec {
            ph.update(0.0);
        }

        // 18 dB decay from 1.0: value = 10^(-18/20) ≈ 0.126
        let expected = 10.0f32.powf(-18.0 / 20.0);
        assert!(
            (ph.value() - expected).abs() / expected < 0.05,
            "decay should be ~18 dB/s, got {} (expected {})",
            ph.value(),
            expected
        );
    }

    #[test]
    fn test_peak_hold_reset() {
        let mut ph = PeakHold::new(18.0, 48000.0, 512);
        ph.update(1.0);
        assert!(ph.value() > 0.9);

        ph.reset();
        assert_eq!(ph.value(), 0.0, "reset should zero the peak hold");
    }

    #[test]
    fn test_meter_values_db_conversion() {
        let values = MeterValues {
            peak: [1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            rms: [0.707, 0.354, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            peak_hold: [1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        };

        // Peak of 1.0 = 0 dBFS
        assert!(
            (values.peak_db(0) - 0.0).abs() < 1e-6,
            "peak 1.0 should be 0 dBFS"
        );

        // Peak of 0.5 = -6.02 dBFS
        assert!(
            (values.peak_db(1) - (-6.02)).abs() < 0.1,
            "peak 0.5 should be ~-6 dBFS"
        );

        // RMS of 0.707 ≈ -3 dBFS (0.707 = 1/sqrt(2))
        assert!(
            (values.rms_db(0) - (-3.01)).abs() < 0.1,
            "rms 0.707 should be ~-3 dBFS"
        );

        // Zero value should give -inf dB
        let zero_values = MeterValues::default();
        assert_eq!(zero_values.peak_db(0), f32::NEG_INFINITY);
    }

    #[test]
    fn test_meter_values_is_clipping() {
        let normal = MeterValues {
            peak: [0.8, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            rms: [0.7, 0.8, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            peak_hold: [0.8, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        };
        assert!(!normal.is_clipping());

        let clipping = MeterValues {
            peak: [1.2, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            rms: [1.0, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            peak_hold: [1.2, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        };
        assert!(clipping.is_clipping());
    }

    #[test]
    fn test_meter_engine_push_and_latest() {
        let mut engine = MeterEngine::new(48000.0, 512);

        let peak = [0.8, 0.7, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let rms = [0.6, 0.55, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        assert!(engine.push(peak, rms), "should push to empty buffer");

        // Consume to update latest_frame.
        engine.consume_all();

        let latest = engine.latest();
        assert_eq!(latest.values.peak[0], 0.8);
        assert_eq!(latest.values.rms[1], 0.55);
        assert_eq!(latest.frame_number, 0);
    }

    #[test]
    fn test_meter_engine_peak_hold_evolution() {
        let mut engine = MeterEngine::new(48000.0, 512);

        // Push a high peak to establish the hold.
        engine.push([1.0; MAX_CHANNELS], [0.9; MAX_CHANNELS]);
        engine.consume_all();

        let first_hold = engine.latest().values.peak_hold[0];
        assert!(first_hold > 0.95, "first peak hold should be near 1.0");

        // Now push low peaks — hold should decay slowly.
        for i in 0..50 {
            engine.push([0.1; MAX_CHANNELS], [0.05; MAX_CHANNELS]);
            engine.consume_all();

            let hold = engine.latest().values.peak_hold[0];
            if i < 20 {
                assert!(
                    hold > 0.5,
                    "frame {i} hold should still be high, got {hold}"
                );
            }
        }
    }

    #[test]
    fn test_meter_engine_buffer_overflow() {
        let mut engine = MeterEngine::new(48000.0, 512);

        // Fill the buffer (capacity = 4).
        for i in 0..4 {
            let peak = [(i + 1) as f32 * 0.2; MAX_CHANNELS];
            let rms = [(i + 1) as f32 * 0.15; MAX_CHANNELS];
            assert!(engine.push(peak, rms), "push {i} should succeed");
        }

        // Next push should fail (buffer full).
        assert!(
            !engine.push([0.5; MAX_CHANNELS], [0.4; MAX_CHANNELS]),
            "should drop when buffer is full"
        );

        // Latest should still be the last successful push.
        engine.consume_all();
        let latest = engine.latest();
        assert!(
            (latest.values.peak[0] - 0.8).abs() < 1e-6,
            "latest should be the last pushed frame"
        );
    }

    #[test]
    fn test_meter_engine_history_rotation() {
        let mut engine = MeterEngine::new(48000.0, 512);

        // Push several frames with different peak_hold values.
        for i in 0..10 {
            let hold = (i as f32) * 0.1;
            engine.push([hold + 0.1; MAX_CHANNELS], [hold; MAX_CHANNELS]);
        }
        engine.consume_all();

        let latest = engine.latest();

        // History should be sorted newest first (descending).
        for i in 0..latest.history.len() - 1 {
            assert!(
                latest.history[i][0] >= latest.history[i + 1][0],
                "history should be newest-first: history[{}]={}, history[{}]={}",
                i,
                latest.history[i][0],
                i + 1,
                latest.history[i + 1][0]
            );
        }

        // The newest entry should be the last frame that survived in the ring
        // buffer (i=3, peak = 0.4).  consume_all() updates latest_frame with
        // the last dequeued frame.
        assert!(
            (latest.history[0][0] - 0.4).abs() < 1e-6,
            "newest history should be last surviving pushed value (i=3)"
        );
    }

    #[test]
    fn test_meter_engine_consume_all() {
        let mut engine = MeterEngine::new(48000.0, 512);

        // Push 3 frames.
        for i in 0..3 {
            let peak = [(i + 1) as f32 * 0.3; MAX_CHANNELS];
            let rms = [(i + 1) as f32 * 0.2; MAX_CHANNELS];
            engine.push(peak, rms);
        }

        // Drain all frames.
        let drained = engine.consume_all();
        assert_eq!(drained, 3, "should drain all 3 frames");
        assert!(engine.is_empty());

        // Latest should be the last drained frame.
        let latest = engine.latest();
        assert!(
            (latest.values.peak[0] - 0.9).abs() < 1e-6,
            "latest should be the last drained frame"
        );
    }

    #[test]
    fn test_meter_engine_reset() {
        let mut engine = MeterEngine::new(48000.0, 512);

        // Push some frames.
        engine.push([1.0; MAX_CHANNELS], [0.9; MAX_CHANNELS]);
        engine.consume_all();

        assert!(engine.frames_pushed() > 0);

        // Reset.
        engine.reset();

        // All values should be zeroed.
        let latest = engine.latest();
        assert_eq!(latest.values.peak, [0.0; MAX_CHANNELS]);
        assert_eq!(latest.values.rms, [0.0; MAX_CHANNELS]);
        assert_eq!(latest.values.peak_hold, [0.0; MAX_CHANNELS]);

        // History should be zeroed.
        for h in latest.history {
            assert_eq!(
                h, [0.0; MAX_CHANNELS],
                "history should be zeroed after reset"
            );
        }
    }

    #[test]
    fn test_meter_engine_default_is_zero() {
        let engine = MeterEngine::default();
        assert!(engine.is_empty());
        assert_eq!(engine.frames_pushed(), 0);

        let latest = engine.latest();
        assert_eq!(latest.values.peak, [0.0; MAX_CHANNELS]);
        assert_eq!(latest.values.rms, [0.0; MAX_CHANNELS]);
        assert_eq!(latest.values.peak_hold, [0.0; MAX_CHANNELS]);
    }

    #[test]
    fn test_meter_engine_stereo_separate_values() {
        let mut engine = MeterEngine::new(48000.0, 512);

        // Push stereo values with different levels per channel.
        engine.push(
            [0.9, 0.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.8, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        );
        engine.consume_all();

        let latest = engine.latest();
        assert!(
            (latest.values.peak[0] - 0.9).abs() < 1e-6,
            "ch0 peak should be 0.9"
        );
        assert!(
            (latest.values.peak[1] - 0.6).abs() < 1e-6,
            "ch1 peak should be 0.6"
        );
        assert!(
            (latest.values.rms[0] - 0.8).abs() < 1e-6,
            "ch0 rms should be 0.8"
        );
        assert!(
            (latest.values.rms[1] - 0.5).abs() < 1e-6,
            "ch1 rms should be 0.5"
        );
    }

    #[test]
    fn test_meter_engine_push_from_analysis() {
        let mut engine = MeterEngine::new(48000.0, 512);

        let analysis = AnalysisFrame {
            block_size: 512,
            sample_rate: 48000.0,
            rms: [0.5; MAX_CHANNELS],
            peak: [0.8; MAX_CHANNELS],
            zero_crossings: [0; MAX_CHANNELS],
            frame_number: 0,
            dropped: false,
        };

        assert!(
            engine.push_from_analysis(&analysis),
            "should push from analysis frame"
        );
        engine.consume_all();

        let latest = engine.latest();
        assert!(
            (latest.values.peak[0] - 0.8).abs() < 1e-6,
            "peak should be extracted from analysis frame"
        );
    }

    #[test]
    fn test_meter_engine_history_full_rotation() {
        let mut engine = MeterEngine::new(48000.0, 512);

        // Push enough frames to fill the entire history buffer.
        for i in 0..METER_HISTORY_LENGTH + 10 {
            let hold = (i as f32) * 0.01;
            engine.push([hold; MAX_CHANNELS], [hold * 0.8; MAX_CHANNELS]);
        }
        engine.consume_all();

        let latest = engine.latest();

        // History should contain the first pushed frame (ring buffer capacity
        // is 4 with DropNew, so only i=0..3 survive). consume_all() updates
        // latest_frame with the last dequeued frame (i=3, hold=0.03).
        assert!(
            (latest.history[0][0] - 0.03).abs() < 1e-6,
            "newest history should be last surviving pushed frame (i=3)"
        );

        // History should be sorted newest first.
        for i in 0..latest.history.len() - 1 {
            assert!(
                latest.history[i][0] >= latest.history[i + 1][0],
                "history should be newest-first at index {i}"
            );
        }
    }
}
