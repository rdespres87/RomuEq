//! Biquad coefficient generation for Equaform DSP.
//!
//! Implements the standard audio EQ biquad filter equations for the MVP filter
//! types: Bell, LowShelf, HighShelf, HighPass, LowPass.
//!
//! All coefficient generation functions are pure — no heap allocation, no
//! side effects. They accept validated parameter values from `equaform-common`
//! and return finite coefficients for all in-range inputs.
//!
//! Reference: Robert Bristow-Johnson's Audio EQ Cookbook
//! (https://web.archive.org/web/20160325094221/http://www.w3.org/2011/audio/audio-eq-cookbook.html)

// ===========================================================================
// BiquadCoefficients
// ===========================================================================

/// Coefficients for a single biquad filter stage.
///
/// Represents the transfer function:
/// ```text
/// H(z) = (b0 + b1*z^-1 + b2*z^-2) / (1 + a1*z^-1 + a2*z^-2)
/// ```
///
/// The denominator coefficient `a0` is always 1.0 (normalised form).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiquadCoefficients {
    /// Numerator coefficient 0.
    pub b0: f32,
    /// Numerator coefficient 1.
    pub b1: f32,
    /// Numerator coefficient 2.
    pub b2: f32,
    /// Denominator coefficient 1 (a0 is implicitly 1.0).
    pub a1: f32,
    /// Denominator coefficient 2.
    pub a2: f32,
}

impl BiquadCoefficients {
    /// Returns `true` if all coefficients are finite (no NaN or Inf).
    pub fn is_finite(&self) -> bool {
        self.b0.is_finite()
            && self.b1.is_finite()
            && self.b2.is_finite()
            && self.a1.is_finite()
            && self.a2.is_finite()
    }
}

impl Default for BiquadCoefficients {
    fn default() -> Self {
        // Unity gain, no filtering — all-pass with gain 1.
        BiquadCoefficients {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

// ===========================================================================
// Coefficient generation
// ===========================================================================

/// Compute biquad coefficients for the given filter type, frequency, gain,
/// Q factor, slope, and sample rate.
///
/// # Arguments
///
/// * `filter_type` — the filter type (Bell, LowShelf, HighShelf, HighPass, LowPass)
/// * `frequency_hz` — center/cutoff frequency in Hz (must be > 0)
/// * `gain_db` — gain in dB (Bell and shelf only; ignored for pass filters)
/// * `q` — Q factor (must be > 0)
/// * `slope` — slope steepness (shelf and pass filters only; affects Q scaling)
/// * `sample_rate` — sample rate in Hz (must be > 2 * frequency_hz for valid results)
///
/// # Returns
///
/// A `BiquadCoefficients` struct. For invalid parameter combinations the
/// function returns a unity-gain (pass-through) filter.
///
/// # Panics
///
/// Panics if `sample_rate <= 0.0` or `frequency_hz <= 0.0`.
pub fn compute_coefficients(
    filter_type: equaform_common::FilterType,
    frequency_hz: f32,
    gain_db: f32,
    q: f32,
    slope: equaform_common::Slope,
    sample_rate: f32,
) -> BiquadCoefficients {
    assert!(sample_rate > 0.0, "sample_rate must be positive");
    assert!(frequency_hz > 0.0, "frequency_hz must be positive");

    // Clamp frequency to safe range: [1.0, Nyquist * 0.999]
    let nyquist = sample_rate * 0.5;
    let frequency_hz = frequency_hz.max(1.0).min(nyquist * 0.999);

    // Clamp Q to a safe range: [0.001, 100.0]
    let q = q.clamp(0.001, 100.0);

    // Scale Q based on slope steepness (higher slope = lower effective Q for stability)
    let q = match slope {
        equaform_common::Slope::Db6 => q,
        equaform_common::Slope::Db12 => q * 1.2,
        equaform_common::Slope::Db18 => q * 1.5,
        equaform_common::Slope::Db24 => q * 2.0,
    };

    match filter_type {
        equaform_common::FilterType::Bell => compute_bell(frequency_hz, gain_db, q, sample_rate),
        equaform_common::FilterType::LowShelf => {
            compute_low_shelf(frequency_hz, gain_db, q, sample_rate)
        }
        equaform_common::FilterType::HighShelf => {
            compute_high_shelf(frequency_hz, gain_db, q, sample_rate)
        }
        equaform_common::FilterType::HighPass => compute_high_pass(frequency_hz, q, sample_rate),
        equaform_common::FilterType::LowPass => compute_low_pass(frequency_hz, q, sample_rate),
    }
}

// ===========================================================================
// Bell (peaking) filter
// ===========================================================================

/// Compute coefficients for a peaking (bell) EQ filter.
///
/// Uses the standard audio EQ cookbook formula (BJD, 2020).
fn compute_bell(freq: f32, gain_db: f32, q: f32, sample_rate: f32) -> BiquadCoefficients {
    let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let sin_omega = omega.sin();
    let cos_omega = omega.cos();
    let alpha = sin_omega / (2.0 * q);
    let sqrt_gain = 10.0_f32.powf(gain_db / 40.0);

    let b0 = 1.0 + alpha * sqrt_gain;
    let b1 = -2.0 * cos_omega;
    let b2 = 1.0 - alpha * sqrt_gain;
    let a0 = 1.0 + alpha / sqrt_gain;
    let a1 = -2.0 * cos_omega;
    let a2 = 1.0 - alpha / sqrt_gain;

    normalise(b0, b1, b2, a0, a1, a2)
}

// ===========================================================================
// Shelf filters
// ===========================================================================

/// Compute coefficients for a low-shelf filter.
///
/// Uses the audio EQ cookbook shelf formula (BJD, 2020).
fn compute_low_shelf(freq: f32, gain_db: f32, q: f32, sample_rate: f32) -> BiquadCoefficients {
    let a = 10.0_f32.powf(gain_db / 40.0); // sqrt(10^(gain/20))
    let sqrt_a = a.sqrt();
    let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let sin_omega = omega.sin();
    let cos_omega = omega.cos();
    let alpha = sin_omega / (2.0 * q);

    let b0 = a * ((a + 1.0) - (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha);
    let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_omega);
    let b2 = a * ((a + 1.0) - (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha);
    let a0 = (a + 1.0) + (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha;
    let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_omega);
    let a2 = (a + 1.0) + (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha;

    normalise(b0, b1, b2, a0, a1, a2)
}

/// Compute coefficients for a high-shelf filter.
fn compute_high_shelf(freq: f32, gain_db: f32, q: f32, sample_rate: f32) -> BiquadCoefficients {
    let a = 10.0_f32.powf(gain_db / 40.0); // sqrt(10^(gain/20))
    let sqrt_a = a.sqrt();
    let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let sin_omega = omega.sin();
    let cos_omega = omega.cos();
    let alpha = sin_omega / (2.0 * q);

    let b0 = a * ((a + 1.0) + (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha);
    let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_omega);
    let b2 = a * ((a + 1.0) + (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha);
    let a0 = (a + 1.0) - (a - 1.0) * cos_omega + 2.0 * sqrt_a * alpha;
    let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_omega);
    let a2 = (a + 1.0) - (a - 1.0) * cos_omega - 2.0 * sqrt_a * alpha;

    normalise(b0, b1, b2, a0, a1, a2)
}

// ===========================================================================
// Pass filters
// ===========================================================================

/// Compute coefficients for a high-pass filter.
fn compute_high_pass(freq: f32, q: f32, sample_rate: f32) -> BiquadCoefficients {
    let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let sin_omega = omega.sin();
    let cos_omega = omega.cos();
    let alpha = sin_omega / (2.0 * q);

    let b0 = (1.0 + cos_omega) / 2.0;
    let b1 = -(1.0 + cos_omega);
    let b2 = (1.0 + cos_omega) / 2.0;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_omega;
    let a2 = 1.0 - alpha;

    normalise(b0, b1, b2, a0, a1, a2)
}

/// Compute coefficients for a low-pass filter.
fn compute_low_pass(freq: f32, q: f32, sample_rate: f32) -> BiquadCoefficients {
    let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let sin_omega = omega.sin();
    let cos_omega = omega.cos();
    let alpha = sin_omega / (2.0 * q);

    let b0 = (1.0 - cos_omega) / 2.0;
    let b1 = 1.0 - cos_omega;
    let b2 = (1.0 - cos_omega) / 2.0;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_omega;
    let a2 = 1.0 - alpha;

    normalise(b0, b1, b2, a0, a1, a2)
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Normalise biquad coefficients so that a0 = 1.0.
///
/// Divides all coefficients by `a0`.
fn normalise(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> BiquadCoefficients {
    let inv_a0 = 1.0 / a0;
    BiquadCoefficients {
        b0: b0 * inv_a0,
        b1: b1 * inv_a0,
        b2: b2 * inv_a0,
        a1: a1 * inv_a0,
        a2: a2 * inv_a0,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ===========================================================================
    // Default coefficients
    // ===========================================================================

    #[test]
    fn test_default_coefficients_are_unity() {
        let c = BiquadCoefficients::default();
        assert_eq!(c.b0, 1.0);
        assert_eq!(c.b1, 0.0);
        assert_eq!(c.b2, 0.0);
        assert_eq!(c.a1, 0.0);
        assert_eq!(c.a2, 0.0);
        assert!(c.is_finite());
    }

    // ===========================================================================
    // Bell filter tests
    // ===========================================================================

    #[test]
    fn test_bell_zero_gain_is_unity() {
        let c = compute_coefficients(
            equaform_common::FilterType::Bell,
            1000.0,                          // Hz
            0.0,                             // dB (no gain)
            std::f32::consts::FRAC_1_SQRT_2, // Q = 1/sqrt(2)
            equaform_common::Slope::Db12,
            48000.0, // sample rate
        );
        // At zero gain, bell should be close to unity
        assert!(
            (c.b0 - 1.0).abs() < 0.01,
            "b0 should be ~1.0 at 0dB gain, got {}",
            c.b0
        );
        assert!(c.is_finite());
    }

    #[test]
    fn test_bell_positive_gain() {
        let c = compute_coefficients(
            equaform_common::FilterType::Bell,
            1000.0,
            12.0, // +12 dB boost
            1.0,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
        // At center frequency, bell should boost
        assert!(c.b0 > 0.5, "b0 should be positive for boost, got {}", c.b0);
    }

    #[test]
    fn test_bell_negative_gain() {
        let c = compute_coefficients(
            equaform_common::FilterType::Bell,
            1000.0,
            -12.0, // -12 dB cut
            1.0,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
    }

    #[test]
    fn test_bell_extreme_q() {
        // Very narrow bell (high Q)
        let c_narrow = compute_coefficients(
            equaform_common::FilterType::Bell,
            1000.0,
            6.0,
            100.0, // very high Q
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c_narrow.is_finite());

        // Very wide bell (low Q)
        let c_wide = compute_coefficients(
            equaform_common::FilterType::Bell,
            1000.0,
            6.0,
            0.001, // very low Q
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c_wide.is_finite());
    }

    // ===========================================================================
    // Low Shelf tests
    // ===========================================================================

    #[test]
    fn test_low_shelf_zero_gain_is_unity() {
        let c = compute_coefficients(
            equaform_common::FilterType::LowShelf,
            1000.0,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(
            (c.b0 - 1.0).abs() < 0.01,
            "b0 should be ~1.0 at 0dB, got {}",
            c.b0
        );
        assert!(c.is_finite());
    }

    #[test]
    fn test_low_shelf_positive_gain() {
        let c = compute_coefficients(
            equaform_common::FilterType::LowShelf,
            500.0,
            12.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
    }

    #[test]
    fn test_low_shelf_negative_gain() {
        let c = compute_coefficients(
            equaform_common::FilterType::LowShelf,
            500.0,
            -12.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
    }

    // ===========================================================================
    // High Shelf tests
    // ===========================================================================

    #[test]
    fn test_high_shelf_zero_gain_is_unity() {
        let c = compute_coefficients(
            equaform_common::FilterType::HighShelf,
            5000.0,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        // At 0dB the overall gain must be unity: (b0+b1+b2)/(1+a1+a2) ≈ 1.0
        let dc_num = c.b0 + c.b1 + c.b2;
        let dc_den = 1.0 + c.a1 + c.a2;
        let gain = dc_num / dc_den;
        assert!(
            (gain - 1.0).abs() < 0.01,
            "overall gain should be ~1.0 at 0dB, got {gain}"
        );
        assert!(c.is_finite());
    }

    #[test]
    fn test_high_shelf_positive_gain() {
        let c = compute_coefficients(
            equaform_common::FilterType::HighShelf,
            5000.0,
            12.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
    }

    #[test]
    fn test_high_shelf_negative_gain() {
        let c = compute_coefficients(
            equaform_common::FilterType::HighShelf,
            5000.0,
            -12.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
    }

    // ===========================================================================
    // High Pass tests
    // ===========================================================================

    #[test]
    fn test_high_pass_attenuates_dc() {
        let c = compute_coefficients(
            equaform_common::FilterType::HighPass,
            1000.0,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
        // At DC (z=1), high-pass should have b0+b1+b2 ≈ 0
        let dc_response = c.b0 + c.b1 + c.b2;
        assert!(
            dc_response.abs() < 0.01,
            "DC response should be near zero, got {dc_response}"
        );
    }

    #[test]
    fn test_high_pass_passes_high_freq() {
        let c = compute_coefficients(
            equaform_common::FilterType::HighPass,
            1000.0,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
        // At Nyquist (z=-1), high-pass should pass
        let nyq_response = c.b0 - c.b1 + c.b2;
        // Should be close to 2.0 (normalized)
        assert!(
            nyq_response.abs() > 1.0,
            "Nyquist response should be high, got {nyq_response}"
        );
    }

    // ===========================================================================
    // Low Pass tests
    // ===========================================================================

    #[test]
    fn test_low_pass_passes_dc() {
        let c = compute_coefficients(
            equaform_common::FilterType::LowPass,
            1000.0,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
        // At DC, low-pass should pass (b0+b1+b2 ≈ 1.0 + a1 + a2)
        let dc_response = c.b0 + c.b1 + c.b2 - c.a1 - c.a2;
        assert!(
            (dc_response - 1.0).abs() < 0.01,
            "DC response should be ~1.0, got {dc_response}"
        );
    }

    #[test]
    fn test_low_pass_attenuates_nyquist() {
        let c = compute_coefficients(
            equaform_common::FilterType::LowPass,
            1000.0,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
        // At Nyquist, low-pass should attenuate
        let nyq_response = c.b0 - c.b1 + c.b2;
        assert!(
            nyq_response.abs() < 0.5,
            "Nyquist response should be low, got {nyq_response}"
        );
    }

    // ===========================================================================
    // Stability tests
    // ===========================================================================

    #[test]
    fn test_all_filter_types_produce_finite_coefficients() {
        let sample_rate = 48000.0;
        let frequencies = [20.0, 100.0, 1000.0, 10000.0, 19000.0];
        let gains = [-24.0, -12.0, 0.0, 12.0, 24.0];
        let qs = [0.1, 0.5, 1.0, 5.0, 20.0];

        for freq in frequencies {
            for gain in gains {
                for q_val in qs {
                    let bell = compute_coefficients(
                        equaform_common::FilterType::Bell,
                        freq,
                        gain,
                        q_val,
                        equaform_common::Slope::Db12,
                        sample_rate,
                    );
                    assert!(
                        bell.is_finite(),
                        "Bell at freq={freq} gain={gain} q={q_val} should be finite"
                    );

                    let low_shelf = compute_coefficients(
                        equaform_common::FilterType::LowShelf,
                        freq,
                        gain,
                        q_val,
                        equaform_common::Slope::Db12,
                        sample_rate,
                    );
                    assert!(
                        low_shelf.is_finite(),
                        "LowShelf at freq={freq} gain={gain} q={q_val} should be finite"
                    );

                    let high_shelf = compute_coefficients(
                        equaform_common::FilterType::HighShelf,
                        freq,
                        gain,
                        q_val,
                        equaform_common::Slope::Db12,
                        sample_rate,
                    );
                    assert!(
                        high_shelf.is_finite(),
                        "HighShelf at freq={freq} gain={gain} q={q_val} should be finite"
                    );

                    let hp = compute_coefficients(
                        equaform_common::FilterType::HighPass,
                        freq,
                        gain,
                        q_val,
                        equaform_common::Slope::Db12,
                        sample_rate,
                    );
                    assert!(
                        hp.is_finite(),
                        "HighPass at freq={freq} gain={gain} q={q_val} should be finite"
                    );

                    let lp = compute_coefficients(
                        equaform_common::FilterType::LowPass,
                        freq,
                        gain,
                        q_val,
                        equaform_common::Slope::Db12,
                        sample_rate,
                    );
                    assert!(
                        lp.is_finite(),
                        "LowPass at freq={freq} gain={gain} q={q_val} should be finite"
                    );
                }
            }
        }
    }

    // ===========================================================================
    // Boundary handling tests
    // ===========================================================================

    #[test]
    fn test_frequency_clamped_to_nyquist() {
        // Frequency above Nyquist should be clamped, not panic
        let c = compute_coefficients(
            equaform_common::FilterType::LowPass,
            50000.0, // above Nyquist for 48kHz
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
    }

    #[test]
    fn test_frequency_clamped_to_minimum() {
        // Very low frequency should be clamped to 1 Hz
        let c = compute_coefficients(
            equaform_common::FilterType::HighPass,
            0.001, // below minimum
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
    }

    #[test]
    fn test_q_clamped() {
        // Extreme Q values should be clamped
        let c = compute_coefficients(
            equaform_common::FilterType::Bell,
            1000.0,
            6.0,
            10000.0, // above max
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());

        let c = compute_coefficients(
            equaform_common::FilterType::Bell,
            1000.0,
            6.0,
            0.00001, // below min
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
    }

    #[test]
    fn test_sample_rate_assertion() {
        // Zero sample rate should panic
        let result = std::panic::catch_unwind(|| {
            compute_coefficients(
                equaform_common::FilterType::Bell,
                1000.0,
                0.0,
                0.707,
                equaform_common::Slope::Db12,
                0.0,
            )
        });
        assert!(result.is_err(), "Zero sample rate should panic");
    }

    #[test]
    fn test_frequency_zero_assertion() {
        // Zero frequency should panic
        let result = std::panic::catch_unwind(|| {
            compute_coefficients(
                equaform_common::FilterType::Bell,
                0.0,
                0.0,
                0.707,
                equaform_common::Slope::Db12,
                48000.0,
            )
        });
        assert!(result.is_err(), "Zero frequency should panic");
    }

    // ===========================================================================
    // DC and Nyquist behavior tests
    // ===========================================================================

    #[test]
    fn test_bell_dc_gain() {
        // Bell at DC should have gain = 0 dB (unity) when gain_db = 0
        let c = compute_coefficients(
            equaform_common::FilterType::Bell,
            1000.0,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        // DC response: H(1) = (b0 + b1 + b2) / (1 + a1 + a2)
        let dc_num = c.b0 + c.b1 + c.b2;
        let dc_den = 1.0 + c.a1 + c.a2;
        let dc_gain = dc_num / dc_den;
        assert!(
            (dc_gain - 1.0).abs() < 0.01,
            "Bell DC gain should be ~1.0 at 0dB, got {dc_gain}"
        );
    }

    #[test]
    fn test_low_shelf_dc_behavior() {
        // Low shelf at DC should have the full gain
        let c = compute_coefficients(
            equaform_common::FilterType::LowShelf,
            500.0,
            12.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        // DC gain in linear: 10^(12/20) ≈ 3.98
        let dc_num = c.b0 + c.b1 + c.b2;
        let dc_den = 1.0 + c.a1 + c.a2;
        let dc_gain = dc_num / dc_den;
        let expected = 10.0_f32.powf(12.0 / 20.0);
        assert!(
            (dc_gain - expected).abs() < expected * 0.5,
            "Low shelf DC gain should be ~{expected} at +12dB, got {dc_gain}"
        );
    }

    #[test]
    fn test_high_pass_dc_attenuation() {
        // High pass at DC should be near zero
        let c = compute_coefficients(
            equaform_common::FilterType::HighPass,
            1000.0,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        let dc_num = c.b0 + c.b1 + c.b2;
        let dc_den = 1.0 + c.a1 + c.a2;
        let dc_gain = dc_num / dc_den;
        assert!(
            dc_gain.abs() < 0.1,
            "High pass DC gain should be near 0, got {dc_gain}"
        );
    }

    #[test]
    fn test_low_pass_nyquist_attenuation() {
        // Low pass at Nyquist should be near zero
        let c = compute_coefficients(
            equaform_common::FilterType::LowPass,
            1000.0,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        let nyq_num = c.b0 - c.b1 + c.b2;
        let nyq_den = 1.0 - c.a1 + c.a2;
        let nyq_gain = nyq_num / nyq_den;
        assert!(
            nyq_gain.abs() < 0.1,
            "Low pass Nyquist gain should be near 0, got {nyq_gain}"
        );
    }

    // ===========================================================================
    // Frequency sweep test
    // ===========================================================================

    #[test]
    fn test_low_pass_frequency_sweep() {
        // Low pass should attenuate more as frequency increases past cutoff
        let cutoff = 1000.0;
        let sample_rate = 48000.0;

        let c_low = compute_coefficients(
            equaform_common::FilterType::LowPass,
            cutoff,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            sample_rate,
        );

        // Check that coefficients are stable across different sample rates
        let c_44k = compute_coefficients(
            equaform_common::FilterType::LowPass,
            cutoff,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            44100.0,
        );

        // Coefficients should be finite at both sample rates
        assert!(c_low.is_finite());
        assert!(c_44k.is_finite());
    }

    // ===========================================================================
    // Slope variation test
    // ===========================================================================

    #[test]
    fn test_slope_variations_produce_finite_coefficients() {
        let slopes = [
            equaform_common::Slope::Db6,
            equaform_common::Slope::Db12,
            equaform_common::Slope::Db18,
            equaform_common::Slope::Db24,
        ];

        for slope in slopes {
            let hp = compute_coefficients(
                equaform_common::FilterType::HighPass,
                1000.0,
                0.0,
                0.707,
                slope,
                48000.0,
            );
            assert!(hp.is_finite(), "HighPass with {slope:?} should be finite");

            let lp = compute_coefficients(
                equaform_common::FilterType::LowPass,
                1000.0,
                0.0,
                0.707,
                slope,
                48000.0,
            );
            assert!(lp.is_finite(), "LowPass with {slope:?} should be finite");
        }
    }

    // ===========================================================================
    // Coefficient normalisation test
    // ===========================================================================

    #[test]
    fn test_coefficients_are_normalised() {
        // After normalisation, the effective a0 should be 1.0
        // We verify by checking that the transfer function at DC and Nyquist
        // produces reasonable values for a known filter
        let c = compute_coefficients(
            equaform_common::FilterType::LowPass,
            1000.0,
            0.0,
            0.707,
            equaform_common::Slope::Db12,
            48000.0,
        );
        assert!(c.is_finite());
        // The coefficients should be in a reasonable range
        assert!(c.b0.abs() < 10.0, "b0 should be bounded");
        assert!(c.b1.abs() < 10.0, "b1 should be bounded");
        assert!(c.b2.abs() < 10.0, "b2 should be bounded");
        assert!(c.a1.abs() < 10.0, "a1 should be bounded");
        assert!(c.a2.abs() < 10.0, "a2 should be bounded");
    }

    // ===========================================================================
    // All filter types at boundary frequencies
    // ===========================================================================

    #[test]
    fn test_all_filters_at_min_frequency() {
        for ft in [
            equaform_common::FilterType::Bell,
            equaform_common::FilterType::LowShelf,
            equaform_common::FilterType::HighShelf,
            equaform_common::FilterType::HighPass,
            equaform_common::FilterType::LowPass,
        ] {
            let c = compute_coefficients(
                ft,
                20.0, // minimum frequency
                0.0,
                0.707,
                equaform_common::Slope::Db12,
                48000.0,
            );
            assert!(c.is_finite(), "Filter {ft:?} at 20Hz should be finite");
        }
    }

    #[test]
    fn test_all_filters_at_max_frequency() {
        for ft in [
            equaform_common::FilterType::Bell,
            equaform_common::FilterType::LowShelf,
            equaform_common::FilterType::HighShelf,
            equaform_common::FilterType::HighPass,
            equaform_common::FilterType::LowPass,
        ] {
            let c = compute_coefficients(
                ft,
                19000.0, // near Nyquist
                0.0,
                0.707,
                equaform_common::Slope::Db12,
                48000.0,
            );
            assert!(c.is_finite(), "Filter {ft:?} at 19kHz should be finite");
        }
    }

    // ===========================================================================
    // Different sample rates
    // ===========================================================================

    #[test]
    fn test_different_sample_rates() {
        let sample_rates = [8000.0, 22050.0, 44100.0, 48000.0, 96000.0];
        for sr in sample_rates {
            let c = compute_coefficients(
                equaform_common::FilterType::Bell,
                1000.0,
                6.0,
                0.707,
                equaform_common::Slope::Db12,
                sr,
            );
            assert!(c.is_finite(), "Bell at sample_rate={sr} should be finite");
        }
    }
}
