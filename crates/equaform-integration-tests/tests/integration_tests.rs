#![forbid(unsafe_code)]

//! Equaform integration tests.
//!
//! Tests the full pipeline: WAV I/O, CLI preset processing, engine lifecycle,
//! and DSP correctness.

use equaform_cli::{get_preset, list_preset_names, process_file};
use equaform_common::{EqParameters, MAX_BANDS, MAX_CHANNELS};
use equaform_engine::EngineLifecycle;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::fs;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a test WAV file with a sine wave.
fn create_test_wav(
    path: &std::path::Path,
    sample_rate: u32,
    channels: u16,
    frequency_hz: f64,
    duration_secs: f64,
    amplitude: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec)?;
    let num_samples = (duration_secs * sample_rate as f64) as usize;

    for i in 0..num_samples {
        let t = i as f64 / sample_rate as f64;
        let sample = (2.0 * std::f64::consts::PI * frequency_hz * t).sin() * amplitude as f64;

        for _ch in 0..channels {
            let clamped = sample.clamp(-1.0, 1.0);
            let i16_val = (clamped * 32767.0) as i16;
            writer.write_sample(i16_val)?;
        }
    }

    writer.finalize()?;
    Ok(())
}

/// Generate a simple mono test WAV (sine wave).
fn generate_mono_sine(
    path: &std::path::Path,
    sample_rate: u32,
    frequency_hz: f64,
    duration_secs: f64,
    amplitude: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    create_test_wav(path, sample_rate, 1, frequency_hz, duration_secs, amplitude)
}

/// Generate a simple stereo test WAV.
fn generate_stereo_sine(
    path: &std::path::Path,
    sample_rate: u32,
    frequency_hz: f64,
    duration_secs: f64,
    amplitude: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    create_test_wav(path, sample_rate, 2, frequency_hz, duration_secs, amplitude)
}

// ---------------------------------------------------------------------------
// Integration tests: CLI processing
// ---------------------------------------------------------------------------

#[test]
fn test_process_mono_file_default_preset() {
    let tmp_dir = std::env::temp_dir().join("equaform_test_default");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let input = tmp_dir.join("input_mono.wav");
    let output = tmp_dir.join("output_mono.wav");

    generate_mono_sine(&input, 44100, 440.0, 0.1, 0.0).unwrap();

    let eq_params = get_preset("default").unwrap();
    process_file(&input, &output, 44100.0, eq_params).unwrap();

    assert!(output.exists(), "Output file should exist");

    let output_reader = WavReader::open(&output).unwrap();
    assert_eq!(output_reader.spec().sample_rate, 44100);
    assert_eq!(output_reader.spec().channels, 1);
    assert_eq!(output_reader.spec().bits_per_sample, 16);
    // 0.1s at 44.1kHz = 4410 samples
    assert_eq!(output_reader.len(), 4410);

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_process_stereo_file_default_preset() {
    let tmp_dir = std::env::temp_dir().join("equaform_test_stereo");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let input = tmp_dir.join("input_stereo.wav");
    let output = tmp_dir.join("output_stereo.wav");

    generate_stereo_sine(&input, 48000, 440.0, 0.05, 0.5).unwrap();

    let eq_params = get_preset("default").unwrap();
    process_file(&input, &output, 48000.0, eq_params).unwrap();

    assert!(output.exists());

    let output_reader = WavReader::open(&output).unwrap();
    assert_eq!(output_reader.spec().sample_rate, 48000);
    assert_eq!(output_reader.spec().channels, 2);
    // 0.05s at 48kHz = 2400 samples, stereo = 4800 channel-samples
    assert_eq!(output_reader.len(), 4800);

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_process_bass_boost_preset() {
    let tmp_dir = std::env::temp_dir().join("equaform_test_bass");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let input = tmp_dir.join("input.wav");
    let output = tmp_dir.join("output.wav");

    // Generate a low-frequency tone (100 Hz) to test bass boost
    generate_mono_sine(&input, 44100, 100.0, 0.2, 0.3).unwrap();

    let eq_params = get_preset("bass_boost").unwrap();
    process_file(&input, &output, 44100.0, eq_params).unwrap();

    assert!(output.exists());

    let output_reader = WavReader::open(&output).unwrap();
    assert_eq!(output_reader.len(), 8820); // 0.2s * 44100

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_process_vocal_preset() {
    let tmp_dir = std::env::temp_dir().join("equaform_test_vocal");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let input = tmp_dir.join("input.wav");
    let output = tmp_dir.join("output.wav");

    generate_mono_sine(&input, 44100, 1000.0, 0.15, 0.4).unwrap();

    let eq_params = get_preset("vocal").unwrap();
    process_file(&input, &output, 44100.0, eq_params).unwrap();

    assert!(output.exists());
    let output_reader = WavReader::open(&output).unwrap();
    assert_eq!(output_reader.len(), 6615); // 0.15s * 44100

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_process_de_ess_preset() {
    let tmp_dir = std::env::temp_dir().join("equaform_test_deess");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let input = tmp_dir.join("input.wav");
    let output = tmp_dir.join("output.wav");

    // Generate a high-frequency tone (5 kHz) to test de-ess
    generate_mono_sine(&input, 44100, 5000.0, 0.1, 0.5).unwrap();

    let eq_params = get_preset("de_ess").unwrap();
    process_file(&input, &output, 44100.0, eq_params).unwrap();

    assert!(output.exists());
    let output_reader = WavReader::open(&output).unwrap();
    assert_eq!(output_reader.len(), 4410);

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_process_unknown_preset_returns_none() {
    assert!(get_preset("nonexistent_preset").is_none());
}

#[test]
fn test_process_all_presets_exist() {
    for name in list_preset_names() {
        assert!(get_preset(name).is_some(), "Preset '{name}' should exist");
    }
}

// ---------------------------------------------------------------------------
// Integration tests: error handling
// ---------------------------------------------------------------------------

#[test]
fn test_process_missing_input_file() {
    let tmp_dir = std::env::temp_dir().join("equaform_test_missing");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let input = tmp_dir.join("nonexistent.wav");
    let output = tmp_dir.join("output.wav");

    let eq_params = get_preset("default").unwrap();
    let result = process_file(&input, &output, 44100.0, eq_params);

    assert!(
        result.is_err(),
        "Should return error for missing input file"
    );

    let _ = fs::remove_dir_all(&tmp_dir);
}

// ---------------------------------------------------------------------------
// Unit tests: Engine lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_engine_prepare_process_reset() {
    let mut engine = EngineLifecycle::new();

    // Test prepare
    assert!(engine.prepare(44100.0, 512, 2));

    // Set flat EQ parameters
    let mut params = EqParameters::default();
    for band in &mut params.bands {
        band.enabled = false;
    }
    engine.set_parameters(params);

    // Create input/output buffers (stereo, 256 samples)
    let block_size = 256;
    let mut input: Vec<Vec<f32>> = (0..2).map(|_| vec![0.0_f32; block_size]).collect();
    let mut output: Vec<Vec<f32>> = (0..2).map(|_| vec![0.0_f32; block_size]).collect();

    // Fill with a test signal
    for input_ch in input.iter_mut().take(2) {
        for (j, sample) in input_ch.iter_mut().take(block_size).enumerate() {
            *sample =
                ((2.0_f64 * std::f64::consts::PI * 440.0 * j as f64 / 44100.0).sin() * 0.5) as f32;
        }
    }

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();

    // Process
    assert!(engine.process(&input_refs, &mut output_refs));

    // Verify output has content (flat EQ should preserve signal)
    let output_sum: f32 = output[0].iter().map(|s| s.abs()).sum();
    assert!(output_sum > 0.0, "Output should contain signal");

    // Test reset
    engine.reset();

    // Engine should still be usable after reset
    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();
    assert!(engine.process(&input_refs, &mut output_refs));
}

#[test]
fn test_engine_process_with_gain() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(44100.0, 512, 1));

    // Set +6 dB output gain
    let mut params = EqParameters::default();
    params.global.output_gain_db = 6.0;
    engine.set_parameters(params);

    let block_size = 256;
    let input: Vec<Vec<f32>> = (0..1).map(|_| vec![0.1_f32; block_size]).collect();
    let mut output: Vec<Vec<f32>> = (0..1).map(|_| vec![0.0_f32; block_size]).collect();

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));

    // With +6dB gain, amplitude should increase by ~2x
    let input_rms: f32 = input[0].iter().map(|s| s * s).sum::<f32>().sqrt() / block_size as f32;
    let output_rms: f32 = output[0].iter().map(|s| s * s).sum::<f32>().sqrt() / block_size as f32;

    // +6dB = 2x linear gain
    assert!(
        output_rms > input_rms * 1.5,
        "Output RMS ({output_rms:.4}) should be ~2x input RMS ({input_rms:.4})"
    );
}

#[test]
fn test_engine_bypass() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(44100.0, 512, 1));

    // Set bypass to true
    let mut params = EqParameters::default();
    params.global.bypass = true;
    engine.set_parameters(params);

    let block_size = 256;
    let input: Vec<Vec<f32>> = (0..1).map(|_| vec![0.5_f32; block_size]).collect();
    let mut output: Vec<Vec<f32>> = (0..1).map(|_| vec![0.0_f32; block_size]).collect();

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));

    // With bypass, output should equal input (approximately)
    for i in 0..block_size {
        let diff = (output[0][i] - input[0][i]).abs();
        assert!(
            diff < 0.01,
            "Bypass: output[{i}] = {}, input[{i}] = {}, diff = {diff}",
            output[0][i],
            input[0][i]
        );
    }
}

#[test]
fn test_engine_prepare_invalid_params() {
    // Zero sample rate should panic (assertion in prepare)
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut engine = EngineLifecycle::new();
            engine.prepare(0.0, 512, 2);
        }))
        .is_err()
    );

    // Zero channels should panic
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut engine = EngineLifecycle::new();
            engine.prepare(44100.0, 512, 0);
        }))
        .is_err()
    );

    // Too many channels should panic
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut engine = EngineLifecycle::new();
            engine.prepare(44100.0, 512, MAX_CHANNELS + 1);
        }))
        .is_err()
    );
}

#[test]
fn test_engine_process_before_prepare() {
    let mut engine = EngineLifecycle::new();

    // Not yet prepared, process should return false
    let input = [vec![0.0_f32; 64]];
    let mut output = [vec![0.0_f32; 64]];

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(!engine.process(&input_refs, &mut output_refs));
}

#[test]
fn test_engine_process_wrong_channel_count() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(44100.0, 512, 2)); // Prepared for stereo

    // Mono input
    let input = [vec![0.1_f32; 64]];
    let mut output = [vec![0.1_f32; 64]];

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();

    // Should return false (channel mismatch)
    assert!(!engine.process(&input_refs, &mut output_refs));
}

// ---------------------------------------------------------------------------
// Unit tests: DSP correctness
// ---------------------------------------------------------------------------

#[test]
fn test_flat_eq_preserves_signal() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(44100.0, 512, 1));

    // Flat EQ (all bands disabled, no gain change)
    let params = EqParameters::default();
    engine.set_parameters(params);

    let block_size = 512;
    // Generate a sine wave at 1 kHz
    let input: Vec<Vec<f32>> = (0..1)
        .map(|_| {
            (0..block_size)
                .map(|i| {
                    (2.0_f64 * std::f64::consts::PI * 1000.0 * i as f64 / 44100.0).sin() as f32
                })
                .collect()
        })
        .collect();
    let mut output = [vec![0.0_f32; block_size]];

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));

    // Flat EQ should preserve the signal (within numerical precision)
    for i in 0..block_size {
        let diff = (output[0][i] - input[0][i]).abs();
        assert!(
            diff < 1e-5,
            "Flat EQ: sample {i} diff = {diff:.2e}, expected ~0"
        );
    }
}

#[test]
fn test_preset_bands_enabled_correctly() {
    // Verify bass_boost enables only band 0
    let params = get_preset("bass_boost").unwrap();
    assert!(params.bands[0].enabled);
    for i in 1..MAX_BANDS {
        assert!(!params.bands[i].enabled, "Band {i} should be disabled");
    }

    // Verify vocal enables bands 0 and 1
    let params = get_preset("vocal").unwrap();
    assert!(params.bands[0].enabled);
    assert!(params.bands[1].enabled);
    for i in 2..MAX_BANDS {
        assert!(!params.bands[i].enabled, "Band {i} should be disabled");
    }

    // Verify de_ess enables only band 0
    let params = get_preset("de_ess").unwrap();
    assert!(params.bands[0].enabled);
    for i in 1..MAX_BANDS {
        assert!(!params.bands[i].enabled, "Band {i} should be disabled");
    }

    // Verify default disables all bands
    let params = get_preset("default").unwrap();
    for i in 0..MAX_BANDS {
        assert!(!params.bands[i].enabled, "Band {i} should be disabled");
    }
}

#[test]
fn test_engine_process_flat_eq_output() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(44100.0, 512, 1));

    // Flat EQ
    let params = EqParameters::default();
    engine.set_parameters(params);

    let block_size = 256;
    // Generate a sine wave at 1 kHz with amplitude 0.5
    let input: Vec<Vec<f32>> = (0..1)
        .map(|_| {
            (0..block_size)
                .map(|i| {
                    ((2.0_f64 * std::f64::consts::PI * 1000.0 * i as f64 / 44100.0).sin() * 0.5)
                        as f32
                })
                .collect()
        })
        .collect();
    let mut output = [vec![0.0_f32; block_size]];

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));

    // Flat EQ: input and output should be approximately equal
    for i in 0..block_size {
        let diff = (output[0][i] - input[0][i]).abs();
        assert!(diff < 1e-5, "Flat EQ: sample {i} diff = {diff:.2e}");
    }
}

#[test]
fn test_engine_process_gain_applied() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(44100.0, 512, 1));

    // Apply +6 dB gain
    let mut params = EqParameters::default();
    params.global.output_gain_db = 6.0;
    engine.set_parameters(params);

    let block_size = 256;
    let input: Vec<Vec<f32>> = (0..1)
        .map(|_| {
            (0..block_size)
                .map(|i| {
                    ((2.0_f64 * std::f64::consts::PI * 1000.0 * i as f64 / 44100.0).sin() * 0.5)
                        as f32
                })
                .collect()
        })
        .collect();
    let mut output = [vec![0.0_f32; block_size]];

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));

    // +6dB = 2x linear gain
    let input_rms: f32 = input[0].iter().map(|s| s * s).sum::<f32>().sqrt() / block_size as f32;
    let output_rms: f32 = output[0].iter().map(|s| s * s).sum::<f32>().sqrt() / block_size as f32;

    // Allow some tolerance for filter interactions
    assert!(
        output_rms > input_rms * 1.8,
        "Output RMS ({output_rms:.4}) should be ~2x input RMS ({input_rms:.4})"
    );
}

#[test]
fn test_engine_process_bypass_preserves_signal() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(44100.0, 512, 1));

    // Enable bypass
    let mut params = EqParameters::default();
    params.global.bypass = true;
    engine.set_parameters(params);

    let block_size = 256;
    let input: Vec<Vec<f32>> = (0..1)
        .map(|_| {
            (0..block_size)
                .map(|i| {
                    ((2.0_f64 * std::f64::consts::PI * 1000.0 * i as f64 / 44100.0).sin() * 0.7)
                        as f32
                })
                .collect()
        })
        .collect();
    let mut output = [vec![0.0_f32; block_size]];

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));

    // With bypass, output should match input
    for i in 0..block_size {
        let diff = (output[0][i] - input[0][i]).abs();
        assert!(diff < 0.01, "Bypass: sample {i} diff = {diff:.4}");
    }
}

#[test]
fn test_engine_lifecycle_sequence() {
    let mut engine = EngineLifecycle::new();

    // Prepare
    assert!(engine.prepare(44100.0, 512, 2));

    // Process a block
    let block_size = 64;
    let input: Vec<Vec<f32>> = (0..2).map(|_| vec![0.1_f32; block_size]).collect();
    let mut output: Vec<Vec<f32>> = (0..2).map(|_| vec![0.0_f32; block_size]).collect();

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();
    assert!(engine.process(&input_refs, &mut output_refs));

    // Reset
    engine.reset();

    // Process again
    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();
    assert!(engine.process(&input_refs, &mut output_refs));

    // Reprepare with different parameters
    assert!(engine.prepare(48000.0, 256, 1));

    // Process with new mono config
    let input: Vec<Vec<f32>> = (0..1).map(|_| vec![0.2_f32; 128]).collect();
    let mut output: Vec<Vec<f32>> = (0..1).map(|_| vec![0.0_f32; 128]).collect();

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();
    assert!(engine.process(&input_refs, &mut output_refs));
}

#[test]
fn test_engine_block_size_handling() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(44100.0, 512, 1));

    // Process a small block (smaller than the prepared max_block_size)
    let block_size = 64;
    let input: Vec<Vec<f32>> = (0..1).map(|_| vec![0.5_f32; block_size]).collect();
    let mut output: Vec<Vec<f32>> = (0..1).map(|_| vec![0.0_f32; block_size]).collect();

    let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(|v| v.as_mut_slice()).collect();
    assert!(engine.process(&input_refs, &mut output_refs));

    // Output should not be all zeros
    let output_sum: f32 = output[0].iter().map(|s| s.abs()).sum();
    assert!(output_sum > 0.0, "Small block output should contain signal");
}

#[test]
fn test_multiple_process_calls() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(44100.0, 512, 1));

    let block_size = 64;
    let input: Vec<Vec<f32>> = (0..1).map(|_| vec![0.1_f32; block_size]).collect();

    // Process the same input multiple times
    for _ in 0..10 {
        let mut output: Vec<Vec<f32>> = (0..1).map(|_| vec![0.0_f32; block_size]).collect();

        let input_refs: Vec<&[f32]> = input.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            output.iter_mut().map(|v| v.as_mut_slice()).collect();
        assert!(engine.process(&input_refs, &mut output_refs));

        // Output should be consistent
        let output_sum: f32 = output[0].iter().map(|s| s.abs()).sum();
        assert!(
            output_sum > 0.0,
            "Repeated processing should produce output"
        );
    }
}

// ---------------------------------------------------------------------------
// Integration tests: WAV I/O round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_wav_roundtrip_mono() {
    let tmp_dir = std::env::temp_dir().join("equaform_test_roundtrip_mono");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let input = tmp_dir.join("input.wav");
    let output = tmp_dir.join("output.wav");

    // Generate a test signal (1 kHz sine, 0.5s)
    generate_mono_sine(&input, 44100, 1000.0, 0.5, 0.3).unwrap();

    // Read input to get expected values
    let mut input_reader = WavReader::open(&input).unwrap();
    let input_samples: Vec<i16> = input_reader
        .samples::<i16>()
        .filter_map(|s| s.ok())
        .collect();

    // Process with default (flat) preset
    let eq_params = get_preset("default").unwrap();
    process_file(&input, &output, 44100.0, eq_params).unwrap();

    // Read output
    let mut output_reader = WavReader::open(&output).unwrap();
    let output_samples: Vec<i16> = output_reader
        .samples::<i16>()
        .filter_map(|s| s.ok())
        .collect();

    // With flat EQ and 0 dB gain, samples should be approximately equal
    assert_eq!(input_samples.len(), output_samples.len());
    for i in 0..input_samples.len() {
        let diff = (input_samples[i] as i32 - output_samples[i] as i32).abs();
        // Allow small numerical differences from float conversion
        assert!(
            diff < 2,
            "Sample {i}: input={}, output={}, diff={}",
            input_samples[i],
            output_samples[i],
            diff
        );
    }

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_wav_roundtrip_stereo() {
    let tmp_dir = std::env::temp_dir().join("equaform_test_roundtrip_stereo");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let input = tmp_dir.join("input.wav");
    let output = tmp_dir.join("output.wav");

    // Generate stereo test signal (440 Hz, 0.2s)
    generate_stereo_sine(&input, 48000, 440.0, 0.2, 0.4).unwrap();

    // Process with default preset
    let eq_params = get_preset("default").unwrap();
    process_file(&input, &output, 48000.0, eq_params).unwrap();

    // Verify output
    let output_reader = WavReader::open(&output).unwrap();
    assert_eq!(output_reader.spec().channels, 2);
    assert_eq!(output_reader.spec().sample_rate, 48000);
    assert_eq!(output_reader.spec().bits_per_sample, 16);

    let _ = fs::remove_dir_all(&tmp_dir);
}
