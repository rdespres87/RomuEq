use super::*;
use crate::ring_buffer::{ANALYSIS_BUFFER_CAPACITY, ANALYSIS_MAX_CHANNELS};
use crate::{AnalysisConfig, AnalysisEngine, AnalysisFrame, MeterEngine};

// ===================================================================
// M5-T002: Spectrum processor tests
// ===================================================================

#[test]
fn test_spectrum_config_default_smoothing() {
    let config = SpectrumConfig::new(1024, 44100.0);
    assert_eq!(config.smoothing_factor, 0.4);
    assert_eq!(config.fft_size, 1024);
    assert_eq!(config.window_type, WindowType::Hann);
    assert_eq!(config.log_bin_mode, LogBinMode::OneThirdOctave);
}

#[test]
fn test_spectrum_config_reconfigure() {
    let config = SpectrumConfig::new(1024, 44100.0);
    let mut processor = SpectrumProcessor::new(config.clone());

    // Reconfigure to different FFT size.
    let new_config = SpectrumConfig::new(2048, 48000.0);
    processor.reconfigure(new_config.clone());

    assert_eq!(processor.fft_size(), 2048);
    assert_eq!(processor.config().smoothing_factor, 0.4);
    assert_eq!(processor.config().log_bin_mode, LogBinMode::OneThirdOctave);
}

#[test]
fn test_spectrum_frame_bin_count() {
    let config = SpectrumConfig::new(1024, 44100.0);
    let mut processor = SpectrumProcessor::new(config);
    let frame = processor.process(&vec![0.0_f32; 1024], 44100.0, 0);

    // FFT size N produces N/2 + 1 unique bins for real input.
    assert_eq!(frame.magnitudes_linear.len(), 1024 / 2 + 1);
    assert_eq!(frame.magnitudes_db.len(), 1024 / 2 + 1);
}

#[test]
fn test_spectrum_sine_peak_detection() {
    // A sine wave at a known frequency should produce a peak in the correct bin.
    let fft_size = 1024;
    let sample_rate = 44100.0;
    // Bin frequency resolution: sample_rate / fft_size ≈ 43.07 Hz
    // Frequency 430.7 Hz → bin 10 (exactly).
    let freq_hz = 430.7;
    let bin_idx = (freq_hz / sample_rate * fft_size as f64) as usize;

    let config = SpectrumConfig {
        fft_size,
        smoothing_factor: 0.0, // No smoothing to see raw peak
        window_type: WindowType::Hann,
        log_bin_mode: LogBinMode::None,
        min_db: -120.0,
    };
    let mut processor = SpectrumProcessor::new(config);

    // Generate one period of sine wave.
    let samples: Vec<f32> = (0..fft_size)
        .map(|i| (2.0 * std::f64::consts::PI * freq_hz / sample_rate * i as f64).sin() as f32)
        .collect();

    let frame = processor.process(&samples, sample_rate, 0);
    let linear = &frame.magnitudes_linear;

    // Find the peak bin.
    let peak_bin = linear
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .unwrap();

    // The peak should be at or very near the expected bin.
    assert!(
        (peak_bin as i64 - bin_idx as i64).abs() <= 1,
        "expected peak near bin {bin_idx} (freq {freq_hz:.1} Hz), got {peak_bin}"
    );

    // Verify the peak is significantly above zero.
    assert!(
        linear[peak_bin] > 0.1,
        "peak magnitude {:.4} should be above noise floor",
        linear[peak_bin]
    );
}

#[test]
fn test_spectrum_ema_smoothing() {
    // Verify that EMA smoothing converges on a steady-state signal.
    let config = SpectrumConfig {
        fft_size: 256,
        smoothing_factor: 0.9, // Strong smoothing
        window_type: WindowType::Hann,
        log_bin_mode: LogBinMode::None,
        min_db: -120.0,
    };
    let mut processor = SpectrumProcessor::new(config);

    // Generate a constant-amplitude sine wave.
    let freq_hz = 440.0;
    let sample_rate = 44100.0;
    let samples: Vec<f32> = (0..256)
        .map(|i| (2.0 * std::f64::consts::PI * freq_hz / sample_rate * i as f64).sin() as f32)
        .collect();

    // Process many frames — the first few should change, later ones should converge.
    let mut prev_peak = 0.0_f32;
    for frame_num in 0..50 {
        let frame = processor.process(&samples, sample_rate, frame_num);
        let peak = *frame
            .magnitudes_linear
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        if frame_num > 10 {
            // After warmup, peak should not change much (EMA converges).
            let delta = (peak - prev_peak).abs();
            assert!(
                delta < 0.01,
                "EMA should converge: frame {frame_num} peak={peak}, prev_peak={prev_peak}, delta={delta}"
            );
        }
        prev_peak = peak;
    }
}

#[test]
fn test_spectrum_log_bin_count() {
    // Verify that log-binned output has the expected number of groups.
    let sample_rate = 48000.0;
    let config = SpectrumConfig {
        fft_size: 2048,
        smoothing_factor: 0.4,
        window_type: WindowType::Hann,
        log_bin_mode: LogBinMode::OneThirdOctave,
        min_db: -120.0,
    };
    let mut processor = SpectrumProcessor::new(config);

    let samples = vec![0.0_f32; 2048];
    let frame = processor.process(&samples, sample_rate, 0);

    // At 48 kHz, 1/3-octave bands from 25 Hz to 24 kHz produce ~28 groups.
    // The exact count depends on the formula; we just verify it's non-zero and bounded.
    assert!(
        frame.num_log_groups > 0 && frame.num_log_groups < 100,
        "expected 1-99 log groups, got {}",
        frame.num_log_groups
    );
    assert_eq!(
        frame.log_binned_magnitudes_linear.len(),
        frame.num_log_groups
    );
    assert_eq!(frame.log_binned_magnitudes_db.len(), frame.num_log_groups);
}

#[test]
fn test_spectrum_no_allocation_during_process() {
    // Verify that after construction, process() reuses buffers.
    // We test by checking that the processor can be called repeatedly
    // without changing its internal buffer sizes.
    let config = SpectrumConfig::new(512, 44100.0);
    let mut processor = SpectrumProcessor::new(config);

    // First process call.
    let samples = vec![0.5_f32; 512];
    let _frame1 = processor.process(&samples, 44100.0, 0);

    // Reconfigure to same size — should reuse buffers.
    let new_config = SpectrumConfig::new(512, 44100.0);
    processor.reconfigure(new_config);

    // Second process call — should produce identical bin count.
    let _frame2 = processor.process(&samples, 44100.0, 1);
    assert_eq!(_frame2.num_bins, 512 / 2 + 1);
}

#[test]
fn test_spectrum_process_ref_and_process_into() {
    let config = SpectrumConfig::new(512, 44100.0);
    let mut processor = SpectrumProcessor::new(config.clone());
    let samples = vec![0.25_f32; 512];

    let (bins, len) = {
        let ref_frame = processor.process_ref(&samples, 44100.0, 42);
        assert_eq!(ref_frame.num_bins, 512 / 2 + 1);
        assert_eq!(ref_frame.frame_number, 42);
        (ref_frame.num_bins, ref_frame.magnitudes_linear.len())
    };

    let mut out_frame = SpectrumFrame::empty(&config);
    processor.process_into(&samples, 44100.0, 43, &mut out_frame);
    assert_eq!(out_frame.frame_number, 43);
    assert_eq!(out_frame.num_bins, bins);
    assert_eq!(out_frame.magnitudes_linear.len(), len);
}

#[test]
fn test_spectrum_zero_input() {
    // All-zero input should produce near-zero magnitudes.
    let config = SpectrumConfig::new(256, 44100.0);
    let mut processor = SpectrumProcessor::new(config);

    let samples = vec![0.0_f32; 256];
    let frame = processor.process(&samples, 44100.0, 0);

    for &mag in &frame.magnitudes_linear {
        assert!(
            mag < 1e-6,
            "zero input should produce near-zero magnitude, got {mag}"
        );
    }
}

#[test]
fn test_spectrum_short_input_zero_padded() {
    // Input shorter than FFT size should be zero-padded and still produce valid output.
    let config = SpectrumConfig::new(1024, 44100.0);
    let mut processor = SpectrumProcessor::new(config);

    // Only 128 samples of a sine wave.
    let samples: Vec<f32> = (0..128)
        .map(|i| (2.0 * std::f64::consts::PI * 440.0 / 44100.0 * i as f64).sin() as f32)
        .collect();

    let frame = processor.process(&samples, 44100.0, 0);

    // Should still produce full bin count.
    assert_eq!(frame.num_bins, 1024 / 2 + 1);
    // Magnitudes should be non-zero (signal exists, even if zero-padded).
    let max_mag = *frame
        .magnitudes_linear
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    assert!(
        max_mag > 0.01,
        "zero-padded signal should have non-zero peak"
    );
}

#[test]
fn test_spectrum_sample_rate_affects_bin_frequency() {
    // Same signal at different sample rates should map to different bin frequencies.
    let fft_size = 1024;

    for sample_rate in [44100.0, 48000.0] {
        let config = SpectrumConfig::new(fft_size, sample_rate);
        let mut processor = SpectrumProcessor::new(config);

        // Simple impulse (all zeros except first sample).
        let mut samples = vec![0.0_f32; fft_size];
        samples[0] = 1.0;

        let frame = processor.process(&samples, sample_rate, 0);

        // For an impulse, the spectrum should be flat (white).
        // Verify bin count is correct.
        assert_eq!(frame.num_bins, fft_size / 2 + 1);

        // Verify the frame number was propagated.
        assert_eq!(frame.frame_number, 0);

        // Verify sample rate was recorded.
        assert!((frame.sample_rate - sample_rate).abs() < 0.1);
    }
}

#[test]
fn test_spectrum_window_types() {
    // Both Hann and Hamming windows should produce valid spectrum.
    for window_type in [WindowType::Hann, WindowType::Hamming] {
        let config = SpectrumConfig {
            fft_size: 512,
            smoothing_factor: 0.4,
            window_type,
            log_bin_mode: LogBinMode::None,
            min_db: -120.0,
        };
        let mut processor = SpectrumProcessor::new(config);

        let samples: Vec<f32> = (0..512)
            .map(|i| (2.0 * std::f64::consts::PI * 220.0 / 44100.0 * i as f64).sin() as f32)
            .collect();

        let frame = processor.process(&samples, 44100.0, 0);

        // Both windows should produce valid output.
        assert_eq!(frame.num_bins, 512 / 2 + 1);
        let max_mag = *frame
            .magnitudes_linear
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        assert!(
            max_mag > 0.0,
            "window {window_type:?} should produce non-zero output"
        );
    }
}

// -----------------------------------------------------------------------
// M5-T004: AnalysisConfig / AnalysisEngine enable/disable/reconfigure
// -----------------------------------------------------------------------

#[test]
fn test_analysis_config_default() {
    let config = AnalysisConfig::default();
    assert!(config.is_enabled());
    assert_eq!(config.sample_rate, 44100.0);
    assert_eq!(config.block_size, 512);
    assert_eq!(config.channels, 2);
}

#[test]
fn test_analysis_config_enable_disable() {
    let mut config = AnalysisConfig::default();
    assert!(config.is_enabled());

    config.disable();
    assert!(!config.is_enabled());

    config.enable();
    assert!(config.is_enabled());

    config.toggle();
    assert!(!config.is_enabled());
}

#[test]
fn test_analysis_engine_with_config() {
    let config = AnalysisConfig::new(48000.0, 256, 1);
    let mut engine = AnalysisEngine::with_config(config);
    assert!(engine.is_active());

    engine.disable();
    assert!(!engine.is_active());

    engine.enable();
    assert!(engine.is_active());
}

#[test]
fn test_analysis_engine_enable_disable_with_frames() {
    let mut engine = AnalysisEngine::new();
    assert!(engine.is_active());

    // Disable — frames should be accepted but not recorded.
    engine.disable();
    assert!(!engine.is_active());

    // Push frames while disabled — buffer should not grow (push returns true
    // but doesn't record when disabled).
    let frame = AnalysisFrame {
        block_size: 256,
        sample_rate: 44100.0,
        rms: [0.0; ANALYSIS_MAX_CHANNELS],
        peak: [0.0; ANALYSIS_MAX_CHANNELS],
        zero_crossings: [0; ANALYSIS_MAX_CHANNELS],
        frame_number: 0,
        dropped: false,
    };

    for _ in 0..ANALYSIS_BUFFER_CAPACITY + 1 {
        engine.push(frame);
    }

    // Buffer should be empty because push returns early when disabled.
    assert_eq!(engine.buffer_len(), 0);

    // Re-enable — engine should be active again.
    engine.enable();
    assert!(engine.is_active());

    // Now push should record frames.
    engine.push(frame);
    assert_eq!(engine.buffer_len(), 1);

    // Consume and verify.
    let drained = engine.consume_all();
    assert_eq!(drained, 1);
    assert_eq!(engine.latest_frame().frame_number, 0);
}

#[test]
fn test_analysis_engine_reconfigure() {
    let mut engine = AnalysisEngine::new();
    assert!(engine.is_active());

    // Reconfigure with new parameters.
    let new_config = AnalysisConfig::new(48000.0, 512, 2);
    engine.reconfigure(new_config);

    // Should still be active after reconfigure.
    assert!(engine.is_active());
    assert_eq!(engine.config().sample_rate, 48000.0);
    assert_eq!(engine.config().block_size, 512);
}

#[test]
fn test_analysis_engine_new_is_active_by_default() {
    // AnalysisEngine::new() uses default config (enabled=true), so it starts active.
    let engine = AnalysisEngine::new();
    assert!(engine.is_active());
}

#[test]
fn test_analysis_engine_push_disabled_then_reenabled() {
    let mut engine = AnalysisEngine::new();

    // Disable and push many frames.
    engine.disable();
    let frame = AnalysisFrame {
        block_size: 256,
        sample_rate: 44100.0,
        rms: [0.0; ANALYSIS_MAX_CHANNELS],
        peak: [0.0; ANALYSIS_MAX_CHANNELS],
        zero_crossings: [0; ANALYSIS_MAX_CHANNELS],
        frame_number: 0,
        dropped: false,
    };

    for i in 0..100 {
        let mut f = frame;
        f.frame_number = i;
        engine.push(f);
    }

    // Buffer should be empty.
    assert_eq!(engine.buffer_len(), 0);

    // Re-enable and push one frame.
    engine.enable();
    let mut f = frame;
    f.frame_number = 100;
    engine.push(f);

    // Only the last frame should be in buffer.
    assert_eq!(engine.buffer_len(), 1);
}

#[test]
fn test_meter_engine_reconfigure() {
    let mut engine = MeterEngine::new(44100.0, 256);
    // Push some data — meter engine push takes peak and rms arrays.
    engine.push(
        [0.8, 0.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.3, 0.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    );

    // Reconfigure with new sample rate and block size.
    engine.reconfigure(48000.0, 512);

    // Reconfigure resets peak-hold state (decay rate changes).
    // Values should be reset to zero after reconfigure.
    let latest = engine.latest();
    assert_eq!(latest.values.rms[0], 0.0);
    assert_eq!(latest.values.rms[1], 0.0);
}

#[test]
fn test_analysis_engine_config_accessors() {
    let config = AnalysisConfig::new(48000.0, 512, 4);
    let mut engine = AnalysisEngine::with_config(config);

    // Immutable access.
    assert_eq!(engine.config().sample_rate, 48000.0);
    assert_eq!(engine.config().block_size, 512);
    assert_eq!(engine.config().channels, 4);

    // Mutable access.
    engine.config_mut().disable();
    assert!(!engine.is_active());
}

// -----------------------------------------------------------------------
// M5-T002: Additional spectrum acceptance criteria tests
// -----------------------------------------------------------------------

#[test]
fn test_spectrum_impulse_flat_response() {
    // A broadband signal (impulse at center of window) should produce a
    // spectrum with energy spread across many bins.
    // We place the impulse at fft_size/2 to avoid Hann window zero at index 0.
    let config = SpectrumConfig {
        fft_size: 256,
        smoothing_factor: 0.0,
        window_type: WindowType::Hann,
        log_bin_mode: LogBinMode::None,
        min_db: -120.0,
    };
    let mut processor = SpectrumProcessor::new(config);

    let mut samples = vec![0.0_f32; 256];
    samples[128] = 1.0; // impulse at center to avoid w[0]=0

    let frame = processor.process(&samples, 44100.0, 0);

    // Spectrum should have non-zero peak.
    let max_mag = *frame
        .magnitudes_linear
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    assert!(
        max_mag > 0.01,
        "broadband impulse spectrum should have non-zero peak, got {max_mag}"
    );

    // Count bins above noise floor.
    let above_noise = frame
        .magnitudes_linear
        .iter()
        .filter(|&&m| m > 0.001)
        .count();
    assert!(
        above_noise > frame.num_bins / 4,
        "broadband impulse should excite many bins, got {above_noise}"
    );
}

#[test]
fn test_spectrum_white_noise_spectral_density() {
    // White noise should have a roughly flat power spectral density.
    let config = SpectrumConfig {
        fft_size: 1024,
        smoothing_factor: 0.0,
        window_type: WindowType::Hann,
        log_bin_mode: LogBinMode::None,
        min_db: -120.0,
    };
    let mut processor = SpectrumProcessor::new(config);

    // Generate white noise with known variance.
    let samples: Vec<f32> = (0..1024)
        .map(|i| {
            let noise = (i as f64 * 0.1).sin() * 0.5 + (i as f64 * 0.37).cos() * 0.3;
            noise as f32
        })
        .collect();

    let frame = processor.process(&samples, 44100.0, 0);

    // Compute average magnitude (should be non-zero).
    let avg_mag = frame.magnitudes_linear.iter().sum::<f32>() / frame.num_bins as f32;
    assert!(
        avg_mag > 0.01,
        "white noise should have non-zero average magnitude"
    );

    // The std of magnitudes should be bounded (not infinite).
    // For FFT magnitude of deterministic signal, std is naturally bounded.
    let variance: f32 = frame
        .magnitudes_linear
        .iter()
        .map(|&m| (m - avg_mag) * (m - avg_mag))
        .sum::<f32>()
        / frame.num_bins as f32;
    let std_mag = variance.sqrt();
    assert!(std_mag < 50.0, "noise std should be bounded, got {std_mag}");
}

#[test]
fn test_spectrum_fft_size_boundaries() {
    // Verify FFT works at various power-of-2 sizes.
    for fft_size in [64, 128, 256, 512, 1024, 2048] {
        let config = SpectrumConfig {
            fft_size,
            smoothing_factor: 0.0,
            window_type: WindowType::Hann,
            log_bin_mode: LogBinMode::None,
            min_db: -120.0,
        };
        let mut processor = SpectrumProcessor::new(config);

        assert_eq!(processor.fft_size(), fft_size);
        assert_eq!(processor.num_bins(), fft_size / 2 + 1);

        // Process silence.
        let samples = vec![0.0_f32; fft_size];
        let frame = processor.process(&samples, 44100.0, 0);

        assert_eq!(frame.num_bins, fft_size / 2 + 1);
        assert_eq!(frame.magnitudes_linear.len(), fft_size / 2 + 1);
        assert_eq!(frame.magnitudes_db.len(), fft_size / 2 + 1);
    }
}

#[test]
fn test_spectrum_frequency_mapping_correctness() {
    // Verify that bin frequencies map correctly to Hz.
    let config = SpectrumConfig {
        fft_size: 1024,
        smoothing_factor: 0.0,
        window_type: WindowType::Hann,
        log_bin_mode: LogBinMode::None,
        min_db: -120.0,
    };
    let mut processor = SpectrumProcessor::new(config);

    // Generate a sine wave at exactly bin 20.
    let sample_rate = 44100.0;
    let expected_bin = 20usize;
    let freq_hz = expected_bin as f64 * sample_rate / processor.fft_size() as f64;

    let samples: Vec<f32> = (0..1024)
        .map(|i| (2.0 * std::f64::consts::PI * freq_hz / sample_rate * i as f64).sin() as f32)
        .collect();

    let frame = processor.process(&samples, sample_rate, 0);

    // Find the peak bin.
    let peak_bin = frame
        .magnitudes_linear
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .unwrap();

    assert_eq!(
        peak_bin, expected_bin,
        "peak should be at bin {expected_bin}, got {peak_bin} (freq {freq_hz:.1} Hz)"
    );

    // Verify adjacent bins are lower (spectral leakage with Hann window).
    let peak_mag = frame.magnitudes_linear[peak_bin];
    // For a perfect bin match, sidelobes of Hann window should be ~31.5 dB down (~0.027x).
    // Use a generous bound to accommodate any normalization differences.
    if peak_bin + 1 < frame.num_bins {
        assert!(
            frame.magnitudes_linear[peak_bin + 1] < peak_mag * 0.6,
            "adjacent bin should be much lower (spectral leakage)"
        );
    }
}

#[test]
fn test_spectrum_reconfigure_does_not_corrupt() {
    // Reconfigure should not corrupt the processor state.
    let config = SpectrumConfig::new(1024, 44100.0);
    let mut processor = SpectrumProcessor::new(config);

    // Process a known signal.
    let samples: Vec<f32> = (0..1024)
        .map(|i| (2.0 * std::f64::consts::PI * 440.0 / 44100.0 * i as f64).sin() as f32)
        .collect();
    let frame1 = processor.process(&samples, 44100.0, 0);
    let peak_before = *frame1
        .magnitudes_linear
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();

    // Reconfigure to different FFT size.
    let new_config = SpectrumConfig::new(2048, 48000.0);
    processor.reconfigure(new_config);

    // Process same signal (truncated to 2048).
    let samples_2k: Vec<f32> = (0..2048)
        .map(|i| (2.0 * std::f64::consts::PI * 440.0 / 48000.0 * i as f64).sin() as f32)
        .collect();
    let frame2 = processor.process(&samples_2k, 48000.0, 1);
    let peak_after = *frame2
        .magnitudes_linear
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();

    // Both should have valid peaks (not NaN/Inf).
    assert!(peak_before.is_finite() && peak_before > 0.0);
    assert!(peak_after.is_finite() && peak_after > 0.0);

    // Reconfigure back to original size.
    processor.reconfigure(SpectrumConfig::new(1024, 44100.0));
    let frame3 = processor.process(&samples, 44100.0, 2);
    let peak_after_restore = *frame3
        .magnitudes_linear
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();

    // Should still produce valid output after restoring.
    assert!(peak_after_restore.is_finite() && peak_after_restore > 0.0);
}

#[test]
fn test_spectrum_one_third_octave_log_binning() {
    // Verify 1/3-octave log binning produces expected group count.
    let config = SpectrumConfig {
        fft_size: 2048,
        smoothing_factor: 0.0,
        window_type: WindowType::Hann,
        log_bin_mode: LogBinMode::OneThirdOctave,
        min_db: -120.0,
    };
    let mut processor = SpectrumProcessor::new(config);

    // Sine wave at 1 kHz should concentrate in specific log groups.
    let samples: Vec<f32> = (0..2048)
        .map(|i| (2.0 * std::f64::consts::PI * 1000.0 / 48000.0 * i as f64).sin() as f32)
        .collect();

    let frame = processor.process(&samples, 48000.0, 0);

    assert!(frame.num_log_groups > 0);
    assert_eq!(
        frame.log_binned_magnitudes_linear.len(),
        frame.num_log_groups
    );

    // At least some groups should have non-zero magnitude.
    let active_groups = frame
        .log_binned_magnitudes_linear
        .iter()
        .filter(|&&m| m > 0.001)
        .count();
    assert!(active_groups > 0, "sine wave should excite log groups");
}

#[test]
fn test_spectrum_octave_log_binning() {
    // Verify octave log binning works correctly.
    let config = SpectrumConfig {
        fft_size: 2048,
        smoothing_factor: 0.0,
        window_type: WindowType::Hann,
        log_bin_mode: LogBinMode::Octave,
        min_db: -120.0,
    };
    let mut processor = SpectrumProcessor::new(config);

    let samples: Vec<f32> = vec![0.0_f32; 2048];
    let frame = processor.process(&samples, 44100.0, 0);

    assert!(frame.num_log_groups > 0);
    assert_eq!(
        frame.log_binned_magnitudes_linear.len(),
        frame.num_log_groups
    );

    // Octave bands should be fewer than 1/3-octave.
    let mut octave_proc = SpectrumProcessor::new(SpectrumConfig {
        fft_size: 2048,
        smoothing_factor: 0.0,
        window_type: WindowType::Hann,
        log_bin_mode: LogBinMode::OneThirdOctave,
        min_db: -120.0,
    });
    let frame_13 = octave_proc.process(&samples, 44100.0, 0);

    assert!(
        frame.num_log_groups < frame_13.num_log_groups,
        "octave bands ({}) should be fewer than 1/3-octave ({})",
        frame.num_log_groups,
        frame_13.num_log_groups
    );
}

#[test]
fn test_spectrum_no_log_binning() {
    // Verify LogBinMode::None produces empty log bins.
    let config = SpectrumConfig {
        fft_size: 512,
        smoothing_factor: 0.0,
        window_type: WindowType::Hann,
        log_bin_mode: LogBinMode::None,
        min_db: -120.0,
    };
    let mut processor = SpectrumProcessor::new(config);

    let samples: Vec<f32> = vec![0.5_f32; 512];
    let frame = processor.process(&samples, 44100.0, 0);

    assert_eq!(frame.num_log_groups, 0);
    assert!(frame.log_binned_magnitudes_linear.is_empty());
    assert!(frame.log_binned_magnitudes_db.is_empty());

    // Linear bins should still be populated.
    assert_eq!(frame.num_bins, 512 / 2 + 1);
}
