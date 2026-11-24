//! Real-time FFT spectrum analysis subsystem.

pub mod frame;
pub mod processor;
pub(crate) mod windowing;

#[cfg(test)]
mod tests;

pub use frame::SpectrumFrame;
pub use processor::SpectrumProcessor;

// =======================================================================
// M5-T002: Spectrum / FFT pipeline (consumer-thread, no audio allocation)
// =======================================================================

/// Window function types supported by the spectrum processor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    /// Hann window (default). Best dynamic range for audio analysis.
    Hann,
    /// Hamming window. Slightly better peak resolution.
    Hamming,
}

/// Frequency bin grouping modes for log-scaled output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogBinMode {
    /// No grouping — one bin per FFT frequency bin (linear).
    None,
    /// Octave bands (1 octave = 2:1 frequency ratio).
    Octave,
    /// One-third octave bands (standard acoustic analysis).
    OneThirdOctave,
}

/// Configuration for the spectrum processor.
#[derive(Debug, Clone)]
pub struct SpectrumConfig {
    /// FFT size (must be power of 2, e.g. 256, 512, 1024, 2048).
    pub fft_size: usize,
    /// Smoothing factor for EMA across frames (0.0 = no smoothing, 1.0 = full).
    /// Default: 0.4.
    pub smoothing_factor: f32,
    /// Window function applied before FFT.
    pub window_type: WindowType,
    /// Log frequency bin grouping mode.
    pub log_bin_mode: LogBinMode,
    /// Minimum dB floor for log magnitude output.
    pub min_db: f32,
}

impl SpectrumConfig {
    /// Creates a new `SpectrumConfig` with the given FFT size and sample rate.
    ///
    /// Uses sensible defaults: Hann window, 1/3-octave log bins, 0.4 EMA
    /// smoothing, -120 dB floor.
    pub fn new(fft_size: usize, _sample_rate: f64) -> Self {
        assert!(
            fft_size.is_power_of_two() && fft_size >= 16,
            "fft_size must be a power of 2 ≥ 16, got {fft_size}"
        );
        Self {
            fft_size,
            smoothing_factor: 0.4,
            window_type: WindowType::Hann,
            log_bin_mode: LogBinMode::OneThirdOctave,
            min_db: -120.0,
        }
    }
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            fft_size: 1024,
            smoothing_factor: 0.4,
            window_type: WindowType::Hann,
            log_bin_mode: LogBinMode::OneThirdOctave,
            min_db: -120.0,
        }
    }
}
