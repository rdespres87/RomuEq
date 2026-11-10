//! Frequency response computation for the EQ cascade.
//!
//! Computes the magnitude response (in dB) of a chain of biquad filters at
//! logarithmically spaced frequency points. This is used by the GUI to render
//! the EQ curve preview and the spectrum analyzer overlay.
//!
//! The transfer function for a single biquad is:
//! ```text
//! H(z) = (b0 + b1*z^-1 + b2*z^-2) / (1 + a1*z^-1 + a2*z^-2)
//! ```
//! Substituting z = e^(jω) with ω = 2πf/fs gives the frequency response.

use crate::coefficients::BiquadCoefficients;
use crate::compute_coefficients;

// ---------------------------------------------------------------------------
// Frequency response point
// ---------------------------------------------------------------------------

/// A single point in a frequency response curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResponsePoint {
    /// Frequency in Hz (logarithmically spaced).
    pub frequency_hz: f32,
    /// Magnitude in decibels.
    pub magnitude_db: f32,
}

// ---------------------------------------------------------------------------
// Single biquad frequency response
// ---------------------------------------------------------------------------

/// Compute the magnitude response of a single biquad filter at one frequency.
///
/// Returns the magnitude in dB (20 * log10(|H(e^(jω))|)).
pub fn biquad_response_at_frequency(
    coeffs: &BiquadCoefficients,
    frequency_hz: f32,
    sample_rate: f32,
) -> f32 {
    if !coeffs.is_finite() || frequency_hz <= 0.0 || frequency_hz >= sample_rate * 0.5 {
        return f32::NEG_INFINITY;
    }

    // Normalized angular frequency: ω = 2π * f / fs
    let omega = 2.0 * std::f32::consts::PI * frequency_hz / sample_rate;
    let cos_w = omega.cos();
    let sin_w = omega.sin();

    // e^(-jω) = cos(ω) - j*sin(ω)
    // e^(-j2ω) = cos(2ω) - j*sin(2ω)
    let cos_2w = (2.0 * omega).cos();
    let sin_2w = (2.0 * omega).sin();

    // Numerator: b0 + b1*e^(-jω) + b2*e^(-j2ω)
    let num_real = coeffs.b0 + coeffs.b1 * cos_w + coeffs.b2 * cos_2w;
    let num_imag = -coeffs.b1 * sin_w - coeffs.b2 * sin_2w;
    let num_mag_sq = num_real * num_real + num_imag * num_imag;

    // Denominator: 1 + a1*e^(-jω) + a2*e^(-j2ω)
    let den_real = 1.0 + coeffs.a1 * cos_w + coeffs.a2 * cos_2w;
    let den_imag = -coeffs.a1 * sin_w - coeffs.a2 * sin_2w;
    let den_mag_sq = den_real * den_real + den_imag * den_imag;

    if den_mag_sq <= 1e-20 {
        return f32::INFINITY;
    }

    // |H| = sqrt(num_mag_sq / den_mag_sq)
    let magnitude = (num_mag_sq / den_mag_sq).sqrt();

    // Convert to dB
    20.0 * magnitude.log10()
}

// ---------------------------------------------------------------------------
// Cascade frequency response
// ---------------------------------------------------------------------------

/// Compute the frequency response of a cascade of enabled biquad filters.
///
/// Generates `num_points` logarithmically spaced points from 20 Hz to Nyquist.
/// Only filters with `enabled == true` contribute to the response.
///
/// # Arguments
///
/// * `sample_rate` — processing sample rate in Hz.
/// * `bands` — array of band parameter sets.
///
/// # Returns
///
/// A vector of `ResponsePoint` covering [20 Hz, Nyquist].
pub fn compute_response(
    sample_rate: f32,
    bands: &[romueq_common::BandParameters; romueq_common::MAX_BANDS],
    num_points: usize,
) -> Vec<ResponsePoint> {
    let nyquist = sample_rate * 0.5;
    let points: Vec<ResponsePoint> = (0..num_points)
        .map(|i| {
            // Logarithmic spacing: f = 20 * (Nyquist/20)^(i/(N-1))
            let log_min = 20.0_f32.log10();
            let log_max = nyquist.log10();
            let t = if num_points == 1 {
                0.5_f32
            } else {
                i as f32 / (num_points - 1) as f32
            };
            let frequency_hz = 10.0_f32.powf(log_min + t * (log_max - log_min));

            // Compute cascade magnitude by multiplying individual magnitudes
            let mut total_mag_linear = 1.0;

            for band in bands.iter() {
                if !band.enabled {
                    continue;
                }

                let coeffs = compute_coefficients(
                    band.filter_type,
                    band.frequency_hz,
                    band.gain_db,
                    band.q,
                    band.slope,
                    sample_rate,
                );

                let mag_db = biquad_response_at_frequency(
                    &coeffs,
                    frequency_hz.min(nyquist * 0.9999),
                    sample_rate,
                );

                if mag_db.is_finite() {
                    total_mag_linear *= 10.0_f32.powf(mag_db / 20.0);
                }
            }

            let total_db = if total_mag_linear > 1e-10 {
                20.0 * total_mag_linear.log10()
            } else {
                f32::NEG_INFINITY
            };

            ResponsePoint {
                frequency_hz,
                magnitude_db: total_db,
            }
        })
        .collect();

    points
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use romueq_common::{BandParameters, FilterType, Slope};

    #[test]
    fn test_single_biquad_response_at_dc() {
        // At DC (very low frequency), a unity gain filter should give 0 dB
        let coeffs = BiquadCoefficients::default();
        let db = biquad_response_at_frequency(&coeffs, 20.0, 48000.0);
        // Default coefficients are [1, 0, 0] / [1, 0, 0] = unity
        assert!(db.abs() < 1.0); // Should be close to 0 dB
    }

    #[test]
    fn test_single_biquad_response_at_nyquist() {
        // At Nyquist, a low-pass filter should attenuate
        let coeffs = compute_coefficients(
            FilterType::LowPass,
            1000.0,
            0.0,
            0.707,
            Slope::Db12,
            48000.0,
        );
        let db = biquad_response_at_frequency(&coeffs, 24000.0, 48000.0);
        // Should be significantly attenuated at Nyquist for a 1 kHz LPF
        assert!(db < -10.0);
    }

    #[test]
    fn test_cascade_response_point_count() {
        let mut bands = [BandParameters::default(); MAX_BANDS];
        // Enable one band at 1 kHz, +6 dB boost
        bands[0] = BandParameters {
            enabled: true,
            filter_type: FilterType::Bell,
            frequency_hz: 1000.0,
            gain_db: 6.0,
            q: 1.0,
            slope: Slope::Db12,
        };

        let response = compute_response(48000.0, &bands, 512);
        assert_eq!(response.len(), 512);
    }

    #[test]
    fn test_cascade_response_boost_at_center() {
        let mut bands = [BandParameters::default(); MAX_BANDS];
        bands[0] = BandParameters {
            enabled: true,
            filter_type: FilterType::Bell,
            frequency_hz: 1000.0,
            gain_db: 12.0,
            q: 0.707,
            slope: Slope::Db12,
        };

        let response = compute_response(48000.0, &bands, 1024);

        // Find the peak — should be near 1000 Hz
        let peak = response
            .iter()
            .max_by(|a, b| a.magnitude_db.partial_cmp(&b.magnitude_db).unwrap())
            .unwrap();

        // Peak should be close to the boost amount
        assert!(
            peak.magnitude_db > 10.0,
            "Peak dB should be > 10, got {}",
            peak.magnitude_db
        );
        // Peak frequency should be near 1000 Hz
        assert!(peak.frequency_hz > 800.0 && peak.frequency_hz < 1200.0);
    }

    #[test]
    fn test_cascade_response_no_enabled_bands() {
        let bands = [BandParameters::default(); MAX_BANDS];

        let response = compute_response(48000.0, &bands, 256);
        assert_eq!(response.len(), 256);

        // All bands disabled → flat response at 0 dB
        for point in &response {
            assert!(point.magnitude_db.abs() < 1.0);
        }
    }

    #[test]
    fn test_logarithmic_spacing() {
        let bands = [BandParameters::default(); MAX_BANDS];
        let response = compute_response(48000.0, &bands, 100);

        // First point should be near 20 Hz
        assert!(response[0].frequency_hz >= 15.0 && response[0].frequency_hz <= 25.0);
        // Last point should be near Nyquist
        assert!(response[99].frequency_hz >= 23000.0);
        // Points should be monotonically increasing
        for i in 1..response.len() {
            assert!(response[i].frequency_hz > response[i - 1].frequency_hz);
        }
    }

    const MAX_BANDS: usize = romueq_common::MAX_BANDS;
}
