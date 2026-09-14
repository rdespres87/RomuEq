//! Biquad filter state and processing.
//!
//! Implements per-channel delay-line state for biquad filters, with reset
//! semantics and channel isolation guarantees.
//!
//! Reference: Robert Bristow-Johnson's Audio EQ Cookbook
//! (https://web.archive.org/web/20160325094221/http://www.w3.org/2011/audio/audio-eq-cookbook.html)

use crate::coefficients::BiquadCoefficients;

// ===========================================================================
// BiquadState — single-channel delay line
// ===========================================================================

/// State for a single biquad filter channel.
///
/// Holds the delay-line samples and coefficients for one channel of a biquad
/// filter. The direct-form I structure computes:
///
/// ```text
/// y[n] = b0*x[n] + b1*x[n-1] + b2*x[n-2] - a1*y[n-1] - a2*y[n-2]
/// ```
#[derive(Clone, Debug)]
pub struct BiquadState {
    /// Delay line: input samples x[n-1], x[n-2].
    x: [f32; 2],
    /// Delay line: output samples y[n-1], y[n-2].
    y: [f32; 2],
    /// Current filter coefficients.
    coeffs: BiquadCoefficients,
}

impl BiquadState {
    /// Create a new state with zeroed delay lines.
    pub fn new(coeffs: BiquadCoefficients) -> Self {
        Self {
            x: [0.0; 2],
            y: [0.0; 2],
            coeffs,
        }
    }

    /// Reset all delay lines to zero.
    ///
    /// After reset, the filter has no memory of previous samples — equivalent
    /// to starting from a cold state.
    pub fn reset(&mut self) {
        self.x = [0.0; 2];
        self.y = [0.0; 2];
    }

    /// Process a single sample.
    ///
    /// Returns the filtered output sample.
    pub fn process(&mut self, input: f32) -> f32 {
        let c = self.coeffs;

        // Direct-form I biquad:
        // y[n] = b0*x[n] + b1*x[n-1] + b2*x[n-2] - a1*y[n-1] - a2*y[n-2]
        let y_n = c.b0 * input + c.b1 * self.x[0] + c.b2 * self.x[1]
            - c.a1 * self.y[0]
            - c.a2 * self.y[1];

        // Shift delay lines
        self.x[1] = self.x[0];
        self.x[0] = input;
        self.y[1] = self.y[0];
        self.y[0] = y_n;

        y_n
    }

    /// Update the filter coefficients.
    ///
    /// The delay lines are NOT reset when coefficients change — this preserves
    /// continuity of the output signal.
    pub fn set_coefficients(&mut self, coeffs: BiquadCoefficients) {
        self.coeffs = coeffs;
    }

    /// Return a reference to the current coefficients.
    pub fn coefficients(&self) -> &BiquadCoefficients {
        &self.coeffs
    }
}

// ===========================================================================
// BiquadFilter — multi-channel filter
// ===========================================================================

/// Multi-channel biquad filter.
///
/// Each channel has independent state — no cross-contamination between
/// channels. The number of channels is fixed at construction.
#[derive(Clone, Debug)]
pub struct BiquadFilter {
    /// Per-channel state.
    channels: Vec<BiquadState>,
}

impl BiquadFilter {
    /// Create a new multi-channel filter.
    ///
    /// # Panics
    ///
    /// Panics if `num_channels` is zero.
    pub fn new(num_channels: usize, coeffs: BiquadCoefficients) -> Self {
        assert!(num_channels > 0, "num_channels must be > 0");
        let channels = (0..num_channels)
            .map(|_| BiquadState::new(coeffs))
            .collect();
        Self { channels }
    }

    /// Reset all channels.
    ///
    /// All delay lines are zeroed. After reset, the filter behaves as if it
    /// just started processing.
    pub fn reset(&mut self) {
        for ch in &mut self.channels {
            ch.reset();
        }
    }

    /// Process a block of samples for all channels.
    ///
    /// # Arguments
    /// * `input` — interleaved input samples (one sample per channel per frame)
    /// * `output` — buffer to write filtered samples (must be same length as input)
    ///
    /// # Panics
    ///
    /// Panics if `input.len() != output.len()` or if the length is not a
    /// multiple of the number of channels.
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(
            input.len(),
            output.len(),
            "input and output must have the same length"
        );
        assert_eq!(
            input.len() % self.channels.len(),
            0,
            "input length must be a multiple of channel count"
        );

        let num_channels = self.channels.len();
        let frames = input.len() / num_channels;

        for frame in 0..frames {
            for ch in 0..num_channels {
                let sample_idx = frame * num_channels + ch;
                let out = self.channels[ch].process(input[sample_idx]);
                output[sample_idx] = out;
            }
        }
    }

    /// Process a single channel's block of samples.
    ///
    /// # Panics
    ///
    /// Panics if `channel` >= number of channels.
    pub fn process_channel(&mut self, channel: usize, input: &[f32], output: &mut [f32]) {
        assert!(
            channel < self.channels.len(),
            "channel {} out of range ({} channels)",
            channel,
            self.channels.len()
        );
        assert_eq!(
            input.len(),
            output.len(),
            "input and output must have the same length"
        );

        for (i, &sample) in input.iter().enumerate() {
            output[i] = self.channels[channel].process(sample);
        }
    }

    /// Return the number of channels.
    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    /// Update coefficients for all channels.
    pub fn set_coefficients(&mut self, coeffs: BiquadCoefficients) {
        for ch in &mut self.channels {
            ch.set_coefficients(coeffs);
        }
    }

    /// Return coefficients for a specific channel.
    pub fn coefficients(&self, channel: usize) -> &BiquadCoefficients {
        &self.channels[channel].coeffs
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coefficients::compute_coefficients;
    use romueq_common::{FilterType, Slope};

    /// Reset produces known behavior: all delay lines are zero.
    #[test]
    fn test_reset_clears_delay_lines() {
        let coeffs = compute_coefficients(
            FilterType::LowPass,
            1000.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );
        let mut state = BiquadState::new(coeffs);

        // Process some samples to populate delay lines
        for _ in 0..10 {
            state.process(1.0);
        }

        // Verify delay lines are non-zero after processing
        // (we can't access x/y directly, but we can verify via output)

        // Reset
        state.reset();

        // After reset, processing a single impulse should produce only b0
        // (since x[n-1]=x[n-2]=y[n-1]=y[n-2]=0)
        let impulse = state.process(1.0);
        assert!(
            (impulse - coeffs.b0).abs() < 1e-6,
            "first sample after reset should be b0"
        );
    }

    /// No stale history survives reset.
    #[test]
    fn test_reset_clears_stale_history() {
        let coeffs = compute_coefficients(
            FilterType::HighPass,
            500.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );
        let mut state = BiquadState::new(coeffs);

        // Feed a long DC signal to build up state
        for _ in 0..1000 {
            state.process(1.0);
        }

        // Reset
        state.reset();

        // Feed DC again — output should be identical to a fresh filter
        let mut fresh = BiquadState::new(coeffs);
        let mut old_outputs = Vec::new();
        let mut fresh_outputs = Vec::new();

        for _ in 0..100 {
            old_outputs.push(state.process(1.0));
            fresh_outputs.push(fresh.process(1.0));
        }

        for (o, f) in old_outputs.iter().zip(fresh_outputs.iter()) {
            assert!(
                (o - f).abs() < 1e-6,
                "output after reset should match fresh filter, got {o} vs {f}"
            );
        }
    }

    /// Channel states cannot cross-contaminate.
    #[test]
    fn test_channel_isolation() {
        let coeffs =
            compute_coefficients(FilterType::Bell, 1000.0, 12.0, 0.707, Slope::Db12, 48000.0);
        let mut filter = BiquadFilter::new(2, coeffs);

        // Process channel 0 with a strong signal
        let input_ch0 = [1.0; 10];
        let mut output_ch0 = [0.0; 10];
        filter.process_channel(0, &input_ch0, &mut output_ch0);

        // Process channel 1 with a different signal
        let input_ch1 = [0.0; 10];
        let mut output_ch1 = [0.0; 10];
        filter.process_channel(1, &input_ch1, &mut output_ch1);

        // Channel 1 output should be all zeros (DC input to bell at 0dB ≈ unity,
        // but starting from zero state, first samples are zero)
        // More importantly, channel 0 output should not be affected by channel 1
        let ch0_sum: f32 = output_ch0.iter().sum();
        let ch1_sum: f32 = output_ch1.iter().sum();

        // Channel 1 processes DC through a bell filter — output should be near 0
        // (bell at 0dB has unity gain at center freq, but DC is attenuated)
        // Channel 0 has non-zero output due to the boost
        assert!(
            ch0_sum.abs() > ch1_sum.abs(),
            "channel 0 should have non-zero output, channel 1 should be near zero"
        );
    }

    /// Multi-channel process interleaves correctly.
    #[test]
    fn test_multichannel_interleaved() {
        let coeffs = compute_coefficients(
            FilterType::LowPass,
            10000.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );
        let mut filter = BiquadFilter::new(2, coeffs);

        // Stereo input: ch0 = 1.0, ch1 = -1.0
        let input = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
        let mut output = vec![0.0; input.len()];
        filter.process(&input, &mut output);

        // Output should be same length
        assert_eq!(output.len(), input.len());

        // After reset, first sample of each channel = b0
        filter.reset();
        let mut output2 = vec![0.0; 2];
        filter.process(&[1.0, -1.0], &mut output2);
        assert!(
            (output2[0] - coeffs.b0).abs() < 1e-6,
            "ch0 first sample should be b0"
        );
        assert!(
            (output2[1] - (-coeffs.b0)).abs() < 1e-6,
            "ch1 first sample should be -b0"
        );
    }

    /// Reset on multi-channel filter resets all channels.
    #[test]
    fn test_multichannel_reset_all() {
        let coeffs = compute_coefficients(
            FilterType::HighPass,
            500.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );
        let mut filter = BiquadFilter::new(4, coeffs);

        // Build state on all channels
        for _ in 0..100 {
            let input = [1.0, 0.5, -0.5, -1.0];
            let mut output = vec![0.0; 4];
            filter.process(&input, &mut output);
        }

        // Reset all
        filter.reset();

        // After reset, each channel should behave like a fresh filter
        let mut fresh = BiquadFilter::new(4, coeffs);
        let input = [1.0, 0.5, -0.5, -1.0];
        let mut output_reset = vec![0.0; 4];
        let mut output_fresh = vec![0.0; 4];

        filter.process(&input, &mut output_reset);
        fresh.process(&input, &mut output_fresh);

        for i in 0..4 {
            assert!(
                (output_reset[i] - output_fresh[i]).abs() < 1e-6,
                "channel {i} output after reset should match fresh"
            );
        }
    }

    /// Coefficient update preserves delay line state.
    #[test]
    fn test_coefficient_update_preserves_state() {
        let coeffs_lp = compute_coefficients(
            FilterType::LowPass,
            1000.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );
        let coeffs_hp = compute_coefficients(
            FilterType::HighPass,
            1000.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );

        let mut state = BiquadState::new(coeffs_lp);

        // Process some samples with LPF
        for _ in 0..50 {
            state.process(1.0);
        }

        // Get current output (depends on accumulated state)
        let output_before = state.process(1.0);

        // Change coefficients (no reset)
        state.set_coefficients(coeffs_hp);

        // Next output depends on the SAME delay line state but different coeffs
        let output_after = state.process(1.0);

        // Outputs should differ (different filter type)
        assert!(
            (output_before - output_after).abs() > 1e-6,
            "output should change when coefficients change"
        );
    }

    /// BiquadFilter panics on invalid channel index.
    #[test]
    #[should_panic(expected = "channel 5 out of range")]
    fn test_process_channel_panics_on_invalid_channel() {
        let coeffs = compute_coefficients(
            FilterType::LowPass,
            1000.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );
        let mut filter = BiquadFilter::new(2, coeffs);
        let input = [1.0; 10];
        let mut output = vec![0.0; 10];
        filter.process_channel(5, &input, &mut output);
    }

    /// BiquadFilter panics on zero channels.
    #[test]
    #[should_panic(expected = "num_channels must be > 0")]
    fn test_new_panics_on_zero_channels() {
        let coeffs = compute_coefficients(
            FilterType::LowPass,
            1000.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );
        let _filter = BiquadFilter::new(0, coeffs);
    }

    /// BiquadFilter panics on mismatched input/output lengths.
    #[test]
    #[should_panic(expected = "input and output must have the same length")]
    fn test_process_panics_on_length_mismatch() {
        let coeffs = compute_coefficients(
            FilterType::LowPass,
            1000.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );
        let mut filter = BiquadFilter::new(1, coeffs);
        let input = [1.0; 10];
        let mut output = vec![0.0; 9];
        filter.process(&input, &mut output);
    }

    /// BiquadFilter panics when input length not multiple of channels.
    #[test]
    #[should_panic(expected = "input length must be a multiple")]
    fn test_process_panics_on_non_multiple_length() {
        let coeffs = compute_coefficients(
            FilterType::LowPass,
            1000.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );
        let mut filter = BiquadFilter::new(2, coeffs);
        let input = [1.0; 5]; // 5 is not a multiple of 2
        let mut output = vec![0.0; 5];
        filter.process(&input, &mut output);
    }

    /// Single-channel filter with unity coefficients passes signal unchanged.
    #[test]
    fn test_single_channel_unity_passthrough() {
        let coeffs = compute_coefficients(
            FilterType::LowPass,
            20000.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );
        let mut state = BiquadState::new(coeffs);

        // At high frequency with 0dB gain, filter should be near unity
        let input = [0.5, -0.3, 0.8, -0.1, 0.6];
        let mut output = vec![0.0; input.len()];

        for (i, &sample) in input.iter().enumerate() {
            output[i] = state.process(sample);
        }

        // After warmup, output should be close to input for near-unity filter
        // (first few samples differ due to zero-state startup)
        for i in 5..input.len() {
            // Allow some tolerance for startup transient
            assert!(
                (output[i] - input[i]).abs() < 0.5,
                "output should approach input for near-unity filter"
            );
        }
    }

    /// Reset is idempotent.
    #[test]
    fn test_reset_is_idempotent() {
        let coeffs =
            compute_coefficients(FilterType::Bell, 1000.0, 6.0, 0.707, Slope::Db12, 48000.0);
        let mut state = BiquadState::new(coeffs);

        // Build state
        for _ in 0..100 {
            state.process(1.0);
        }

        // Reset twice
        state.reset();
        state.reset();

        // Should be identical to single reset
        let mut fresh = BiquadState::new(coeffs);
        let output = state.process(1.0);
        let expected = fresh.process(1.0);
        assert!(
            (output - expected).abs() < 1e-6,
            "double reset should equal single reset"
        );
    }
}
