//! Perceptual loudness-weighted and headroom-safe auto-gain compensation.

use equaform_common::{
    AutoGainMode, BandParameters, BandRouting, EqParameters, FilterType, MAX_BANDS,
};
use equaform_dsp::biquad_response_at_frequency;
use equaform_dsp::coefficients::{BiquadCoefficients, compute_coefficients};

/// Number of logarithmically spaced frequency samples across the audible band.
const NUM_POINTS: usize = 64;

/// Musical headroom scaling applied to the spectrally weighted positive EQ peak.
const MUSICAL_HEADROOM_FACTOR: f32 = 0.75;

/// Loudness contribution weight by per-band stereo routing.
#[inline]
const fn loudness_routing_weight(routing: BandRouting) -> f32 {
    match routing {
        BandRouting::Stereo | BandRouting::Mid => 1.0,
        BandRouting::Left | BandRouting::Right => 0.5,
        BandRouting::Side => 0.35,
    }
}

/// Universal musical peak-energy spectral envelope weight in `[0.35, 1.0]`.
///
/// Full weight across `80 Hz..=4.5 kHz` where broadband musical transients peak,
/// with smooth roll-off in deep sub-bass (`< 80 Hz`) and upper treble (`> 4.5 kHz`)
/// where real music spectral energy naturally tapers off.
#[inline]
fn musical_peak_envelope(freq: f32) -> f32 {
    let sub = if freq < 80.0 {
        let t = ((freq - 20.0) / 60.0).clamp(0.0, 1.0);
        0.35 + 0.65 * t
    } else {
        1.0
    };
    let air = if freq > 4_500.0 {
        let octaves_above = (freq / 4_500.0).log2();
        (1.0 / (1.0 + 0.35 * octaves_above)).max(0.50)
    } else {
        1.0
    };
    sub * air
}

/// Smooth continuous perceptual frequency weight approximating ear sensitivity
/// across 20 Hz to 20 kHz (rolls off sub-bass and ultra-highs, emphasizes 1.5-4 kHz).
#[inline]
fn perceptual_frequency_weight(freq: f32) -> f32 {
    let low_ratio = (freq / 80.0).powi(2);
    let low = low_ratio / (1.0 + low_ratio);

    let high_ratio = (freq / 10_000.0).powi(2);
    let high = 1.0 / (1.0 + high_ratio);

    let mid_delta = (freq.log10() - 3.3) / 0.45;
    let mid = 1.0 + 0.5 / (1.0 + mid_delta * mid_delta);

    (low * high * mid).max(0.05)
}

/// Recompute perceptual auto-gain compensation in dB for the given bands and
/// sample rate using default stereo routing and `AutoGainMode::Perceptual`.
pub fn calculate_auto_gain_compensation_db(
    bands: &[BandParameters; MAX_BANDS],
    sample_rate: f32,
) -> f32 {
    let eq = EqParameters {
        bands: *bands,
        ..EqParameters::default()
    };
    calculate_auto_gain_for_params(&eq, sample_rate)
}

/// Recompute auto-gain compensation in dB for the full EQ parameter set,
/// respecting per-band stereo routing and the selected `AutoGainMode`.
///
/// - `AutoGainMode::Perceptual`: symmetric perceptually weighted mean in dB,
///   excluding High-Pass and Low-Pass utility filters so band-limiting never
///   artificially boosts output gain.
/// - `AutoGainMode::HeadroomSafe`: caps output gain by the worst-case per-channel
///   positive EQ peak across all enabled bands so boosts never clip 0 dBFS audio.
pub fn calculate_auto_gain_for_params(eq: &EqParameters, sample_rate: f32) -> f32 {
    if !sample_rate.is_finite() || sample_rate <= 0.0 {
        return 0.0;
    }

    let mut coeffs = [BiquadCoefficients::default(); MAX_BANDS];
    for (slot, bp) in coeffs.iter_mut().zip(eq.bands.iter()) {
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

    let f_min = 20.0_f32;
    let f_max = (sample_rate * 0.45).min(20_000.0).max(f_min);
    let log_min = f_min.log10();
    let log_span = f_max.log10() - log_min;

    let mut sum_weighted_db = 0.0_f32;
    let mut sum_weight = 0.0_f32;
    let mut max_positive_db = 0.0_f32;

    for k in 0..NUM_POINTS {
        let t = k as f32 / (NUM_POINTS - 1) as f32;
        let freq = 10.0_f32.powf(log_min + t * log_span);
        let weight = perceptual_frequency_weight(freq);

        let mut perceptual_db = 0.0_f32;
        let mut db_l = 0.0_f32;
        let mut db_r = 0.0_f32;

        for (band_idx, (bp, coeff)) in eq.bands.iter().zip(coeffs.iter()).enumerate() {
            if !bp.enabled {
                continue;
            }
            let band_db = biquad_response_at_frequency(coeff, freq, sample_rate);
            if !band_db.is_finite() {
                continue;
            }
            let routing = eq.band_routing(band_idx);

            if !matches!(bp.filter_type, FilterType::HighPass | FilterType::LowPass) {
                perceptual_db += band_db * loudness_routing_weight(routing);
            }

            match routing {
                BandRouting::Stereo | BandRouting::Mid => {
                    db_l += band_db;
                    db_r += band_db;
                }
                BandRouting::Left => {
                    db_l += band_db;
                }
                BandRouting::Right => {
                    db_r += band_db;
                }
                BandRouting::Side => {
                    let side_db = band_db * std::f32::consts::FRAC_1_SQRT_2;
                    db_l += side_db;
                    db_r += side_db;
                }
            }
        }

        sum_weighted_db += weight * perceptual_db.clamp(-36.0, 36.0);
        sum_weight += weight;
        let channel_peak_db = db_l.max(db_r).max(0.0);
        max_positive_db = max_positive_db.max(channel_peak_db * musical_peak_envelope(freq));
    }

    let mean_db = if sum_weight > 0.0 {
        sum_weighted_db / sum_weight
    } else {
        0.0
    };

    match eq.global.auto_gain_mode {
        AutoGainMode::Perceptual => (-mean_db).clamp(-15.0, 12.0),
        AutoGainMode::HeadroomSafe => (-mean_db)
            .min(-(max_positive_db * MUSICAL_HEADROOM_FACTOR))
            .clamp(-24.0, 12.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_eq() -> EqParameters {
        let mut eq = EqParameters::default();
        for b in &mut eq.bands {
            b.enabled = false;
        }
        eq
    }

    #[test]
    fn test_flat_and_invalid_sample_rate_return_zero() {
        let eq = flat_eq();
        assert_eq!(calculate_auto_gain_for_params(&eq, 48_000.0), 0.0);
        assert_eq!(calculate_auto_gain_for_params(&eq, 0.0), 0.0);
        assert_eq!(calculate_auto_gain_for_params(&eq, f32::NAN), 0.0);
    }

    #[test]
    fn test_perceptual_mode_is_symmetric_for_boost_and_cut() {
        let mut boost = flat_eq();
        boost.bands[0].enabled = true;
        boost.bands[0].filter_type = FilterType::Bell;
        boost.bands[0].frequency_hz = 1000.0;
        boost.bands[0].gain_db = 12.0;
        boost.bands[0].q = 1.0;

        let mut cut = boost;
        cut.bands[0].gain_db = -12.0;

        let comp_boost = calculate_auto_gain_for_params(&boost, 48_000.0);
        let comp_cut = calculate_auto_gain_for_params(&cut, 48_000.0);

        assert!(
            comp_boost < -1.0,
            "expected negative comp for boost, got {comp_boost}"
        );
        assert!(
            (comp_boost + comp_cut).abs() < 0.05,
            "boost ({comp_boost}) and cut ({comp_cut}) should be symmetric"
        );
    }

    #[test]
    fn test_hpf_does_not_artificially_boost_auto_gain() {
        let mut eq = flat_eq();
        eq.bands[0].enabled = true;
        eq.bands[0].filter_type = FilterType::HighPass;
        eq.bands[0].frequency_hz = 100.0;
        eq.bands[0].q = std::f32::consts::FRAC_1_SQRT_2;

        let comp_perceptual = calculate_auto_gain_for_params(&eq, 48_000.0);
        assert!(
            comp_perceptual.abs() < 1e-4,
            "HPF should not boost perceptual auto-gain, got {comp_perceptual}"
        );
    }

    #[test]
    fn test_headroom_safe_protects_against_peak_clipping() {
        let mut eq = flat_eq();
        eq.global.auto_gain_mode = AutoGainMode::HeadroomSafe;
        eq.bands[0].enabled = true;
        eq.bands[0].filter_type = FilterType::Bell;
        eq.bands[0].frequency_hz = 1000.0;
        eq.bands[0].gain_db = 6.0;
        eq.bands[0].q = 2.0;

        let comp_safe = calculate_auto_gain_for_params(&eq, 48_000.0);
        assert!(
            (-4.8..=-4.2).contains(&comp_safe),
            "musical headroom-safe should attenuate ~-4.5 dB for +6 dB mid peak, got {comp_safe}"
        );
    }

    #[test]
    fn test_headroom_safe_tapers_sub_bass_and_air_boosts() {
        let mut mid = flat_eq();
        mid.global.auto_gain_mode = AutoGainMode::HeadroomSafe;
        mid.bands[0].enabled = true;
        mid.bands[0].filter_type = FilterType::Bell;
        mid.bands[0].frequency_hz = 1000.0;
        mid.bands[0].gain_db = 6.0;
        mid.bands[0].q = 2.0;

        let mut air = mid;
        air.bands[0].frequency_hz = 10_000.0;

        let mut sub_shelf = flat_eq();
        sub_shelf.global.auto_gain_mode = AutoGainMode::HeadroomSafe;
        sub_shelf.bands[0].enabled = true;
        sub_shelf.bands[0].filter_type = FilterType::LowShelf;
        sub_shelf.bands[0].frequency_hz = 56.0;
        sub_shelf.bands[0].gain_db = 12.6;
        sub_shelf.bands[0].q = std::f32::consts::FRAC_1_SQRT_2;

        let comp_mid = calculate_auto_gain_for_params(&mid, 48_000.0);
        let comp_air = calculate_auto_gain_for_params(&air, 48_000.0);
        let comp_sub = calculate_auto_gain_for_params(&sub_shelf, 48_000.0);

        assert!(
            comp_air > comp_mid + 0.8 && comp_air < -2.0,
            "10 kHz air boost ({comp_air}) should attenuate less than 1 kHz mid boost ({comp_mid})"
        );
        assert!(
            (-5.2..=-3.2).contains(&comp_sub),
            "deep sub-bass shelf (+12.6 dB @ 56 Hz) should yield balanced ~-4.4 dB, got {comp_sub}"
        );
    }

    #[test]
    fn test_left_only_routing_weights_loudness_half_but_protects_channel_peak() {
        let mut eq = flat_eq();
        eq.bands[0].enabled = true;
        eq.bands[0].filter_type = FilterType::Bell;
        eq.bands[0].frequency_hz = 1000.0;
        eq.bands[0].gain_db = 6.0;
        eq.bands[0].q = 1.0;

        let stereo_comp = calculate_auto_gain_for_params(&eq, 48_000.0);

        eq.set_band_routing(0, BandRouting::Left);
        let left_perceptual = calculate_auto_gain_for_params(&eq, 48_000.0);
        assert!(
            (left_perceptual - stereo_comp * 0.5).abs() < 0.05,
            "Left-only perceptual comp ({left_perceptual}) should be half of stereo ({stereo_comp})"
        );

        eq.global.auto_gain_mode = AutoGainMode::HeadroomSafe;
        let left_safe = calculate_auto_gain_for_params(&eq, 48_000.0);
        assert!(
            (-4.8..=-4.2).contains(&left_safe),
            "Left-only HeadroomSafe must still protect L channel peak (~-4.5 dB), got {left_safe}"
        );
    }
}
