//! Window functions and logarithmic octave / 1/3-octave band edge calculations.

// =======================================================================
// Window functions
// =======================================================================

/// Computes a Hann window of the given size.
pub(crate) fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / size as f32).cos()))
        .collect()
}

/// Computes a Hamming window of the given size.
pub(crate) fn hamming_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / size as f32).cos())
        .collect()
}

// =======================================================================
// Log frequency bin helpers (IEC 61260 standard)
// =======================================================================

/// Returns the number of octave bands that fit within [0, sample_rate/2].
pub(crate) fn octave_groups(_fft_size: usize, sample_rate: f64) -> usize {
    let nyquist = sample_rate / 2.0;
    // Center frequencies start at 25.5 Hz, double each step.
    let base = 25.5;
    if nyquist <= base {
        return 1;
    }
    ((nyquist / base).log2().floor() as usize) + 2
}

/// Returns the number of 1/3-octave bands that fit within [0, sample_rate/2].
pub(crate) fn one_third_octave_groups(_fft_size: usize, sample_rate: f64) -> usize {
    let nyquist = sample_rate / 2.0;
    // Center frequencies: 25, 31.5, 40, 50, 63, 80, 100, ... Hz
    let base = 25.0;
    if nyquist <= base {
        return 1;
    }
    (((nyquist / base).log2() * 3.0).floor() as i64 + 11).max(1) as usize
}

/// Returns the band edges for an octave group index.
pub(crate) fn octave_group_edges(group: usize) -> Option<(f64, f64)> {
    let base = 25.5;
    let low = base * (group as f64).max(1.0);
    let high = low * 2.0;
    Some((low, high))
}

/// Returns the band edges for a 1/3-octave group index.
pub(crate) fn one_third_octave_group_edges(group: usize) -> Option<(f64, f64)> {
    // Standard 1/3-octave centers: group 10 = 25 Hz, group 13 = 31.5 Hz, etc.
    // f_center = base * 2^(n/3) where n = group - 10
    let base = 25.0;
    let n = (group as i64) - 10;
    let center = base * (2.0_f64).powf(n as f64 / 3.0);
    let low = center / 2.0_f64.powf(1.0 / 6.0);
    let high = center * 2.0_f64.powf(1.0 / 6.0);
    Some((low, high))
}

