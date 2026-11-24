//! Single-frame FFT spectrum output container.

use super::windowing::{
    octave_group_edges, octave_groups, one_third_octave_group_edges, one_third_octave_groups,
};
use super::{LogBinMode, SpectrumConfig};

/// A single frame of spectrum data produced by `SpectrumProcessor`.
#[derive(Debug, Clone)]
pub struct SpectrumFrame {
    /// Number of linear frequency bins (fft_size / 2 + 1).
    pub num_bins: usize,
    /// FFT size used for this frame.
    pub fft_size: usize,
    /// Sample rate used for frequency calculations.
    pub sample_rate: f64,
    /// Linear magnitude per bin (fft_size / 2 + 1 values).
    pub magnitudes_linear: Vec<f32>,
    /// Magnitude in dB per bin (same layout as magnitudes_linear).
    pub magnitudes_db: Vec<f32>,
    /// Log-binned magnitude (one value per group).
    pub log_binned_magnitudes_linear: Vec<f32>,
    /// Log-binned magnitude in dB.
    pub log_binned_magnitudes_db: Vec<f32>,
    /// Number of log groups (0 if log_bin_mode is None).
    pub num_log_groups: usize,
    /// Frame sequence number (matches AnalysisFrame::frame_number).
    pub frame_number: u64,
}

impl SpectrumFrame {
    /// Returns the center frequency of a linear bin.
    #[inline]
    pub fn bin_frequency(&self, bin: usize) -> f64 {
        bin as f64 * self.sample_rate / self.fft_size as f64
    }

    /// Returns the band edges (low, high) for a log group.
    pub fn log_group_edges(&self, group: usize) -> Option<(f64, f64)> {
        if self.num_log_groups == 0 {
            return None;
        }
        match self.log_bin_mode() {
            LogBinMode::None => None,
            LogBinMode::Octave => octave_group_edges(group),
            LogBinMode::OneThirdOctave => one_third_octave_group_edges(group),
        }
    }

    /// Returns the log bin mode used for this frame.
    fn log_bin_mode(&self) -> LogBinMode {
        if self.num_log_groups == 0 {
            LogBinMode::None
        } else {
            // Default to 1/3-octave for non-zero groups (matches default config).
            LogBinMode::OneThirdOctave
        }
    }

    /// Creates an empty frame with the given FFT size and log mode.
    pub(crate) fn empty(config: &SpectrumConfig) -> Self {
        let num_bins = config.fft_size / 2 + 1;
        let (log_linear, log_db, num_groups) = Self::empty_log_bins(config);
        Self {
            num_bins,
            fft_size: config.fft_size,
            sample_rate: 44100.0,
            magnitudes_linear: vec![0.0_f32; num_bins],
            magnitudes_db: vec![config.min_db; num_bins],
            log_binned_magnitudes_linear: log_linear,
            log_binned_magnitudes_db: log_db,
            num_log_groups: num_groups,
            frame_number: 0,
        }
    }

    pub(crate) fn empty_log_bins(config: &SpectrumConfig) -> (Vec<f32>, Vec<f32>, usize) {
        // Use 48 kHz default for construction-time group sizing.
        // Actual frequency mapping uses the runtime sample rate in process().
        const DEFAULT_SAMPLE_RATE: f64 = 48000.0;

        match config.log_bin_mode {
            LogBinMode::None => (Vec::new(), Vec::new(), 0),
            LogBinMode::Octave => {
                let groups = octave_groups(config.fft_size, DEFAULT_SAMPLE_RATE);
                (vec![0.0_f32; groups], vec![config.min_db; groups], groups)
            }
            LogBinMode::OneThirdOctave => {
                let groups = one_third_octave_groups(config.fft_size, DEFAULT_SAMPLE_RATE);
                (vec![0.0_f32; groups], vec![config.min_db; groups], groups)
            }
        }
    }
}
