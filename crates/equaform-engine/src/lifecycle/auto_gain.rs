//! Perceptual loudness-weighted auto-gain compensation.

use equaform_common::{BandParameters, MAX_BANDS};
use equaform_dsp::biquad_response_at_frequency;
use equaform_dsp::coefficients::{BiquadCoefficients, compute_coefficients};

/// Recompute perceptual RMS power loudness-weighted auto-gain compensation in dB
/// for the given bands and sample rate.
pub fn calculate_auto_gain_compensation_db(
    bands: &[BandParameters; MAX_BANDS],
    sample_rate: f32,
) -> f32 {
    let mut coeffs = [BiquadCoefficients::default(); MAX_BANDS];
    for (band_idx, slot) in coeffs.iter_mut().enumerate() {
        let bp = &bands[band_idx];
        if bp.enabled {
            *slot = compute_coefficients(
                bp.filter_type,
                bp.frequency_hz,
                bp.gain_db,
                bp.q,
                bp.slope,
                sample_rate,
            );
        }
    }

    let log_min = 50.0_f32.log10();
    let log_max = 14000.0_f32.log10();
    let mut sum_power = 0.0_f32;
    let mut sum_weight = 0.0_f32;
    for k in 0..24 {
        let t = k as f32 / 23.0;
        let freq = 10.0_f32.powf(log_min + t * (log_max - log_min));
        let mut total_db = 0.0_f32;
        for (band_idx, coeff) in coeffs.iter().enumerate() {
            if bands[band_idx].enabled {
                let band_db = biquad_response_at_frequency(coeff, freq, sample_rate);
                if band_db.is_finite() {
                    total_db += band_db;
                }
            }
        }
        let weight = if (200.0..=5000.0).contains(&freq) {
            1.8_f32
        } else {
            1.0_f32
        };
        let clamped_db = total_db.clamp(-36.0, 36.0);
        sum_power += weight * 10.0_f32.powf(clamped_db / 10.0);
        sum_weight += weight;
    }
    let mean_power = (sum_power / sum_weight).max(1e-6);
    let rms_gain_db = 10.0_f32 * mean_power.log10();
    (-rms_gain_db).clamp(-15.0, 12.0)
}

