use super::*;
use equaform_common::{BandParameters, FilterType, Slope};

// -----------------------------------------------------------------------
// EngineLifecycle construction tests
// -----------------------------------------------------------------------

#[test]
fn test_lifecycle_new_is_created() {
    let engine = EngineLifecycle::new();
    assert_eq!(engine.state(), LifecycleState::Created);
    assert_eq!(engine.sample_rate(), 44100.0);
    assert_eq!(engine.max_block_size(), 512);
    assert_eq!(engine.channels(), 2);
    // Buffer should be all zeros.
    assert!(engine.buffer().iter().all(|&s| s == 0.0));
}

#[test]
fn test_lifecycle_new_no_allocation() {
    // Verify the inline struct size stays bounded (large scratch/delay arrays live behind Box<[f32]>).
    let size = std::mem::size_of::<EngineLifecycle>();
    let expected = FLAT_BUFFER_SIZE * 8
        + DELAY_BUFFER_SIZE * 8
        + std::mem::size_of::<equaform_dsp::LinearPhaseConvolver>();
    // Allow up to 2048 bytes of metadata/padding overhead.
    assert!(
        size >= expected && size <= expected + 2048,
        "EngineLifecycle size {size} out of range [{expected}, {}]",
        expected + 2048
    );
}

// -----------------------------------------------------------------------
// Prepare tests
// -----------------------------------------------------------------------

#[test]
fn test_lifecycle_prepare_mono() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(48000.0, 256, 1));
    assert_eq!(engine.state(), LifecycleState::Ready);
    assert_eq!(engine.sample_rate(), 48000.0);
    assert_eq!(engine.max_block_size(), 256);
    assert_eq!(engine.channels(), 1);
}

#[test]
fn test_lifecycle_prepare_stereo() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(44100.0, 512, 2));
    assert_eq!(engine.state(), LifecycleState::Ready);
    assert_eq!(engine.channels(), 2);
}

#[test]
fn test_lifecycle_prepare_zeroes_buffer() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    // Write some data.
    engine.buffer_mut()[0] = 1.0;
    engine.buffer_mut()[1] = 2.0;

    // Repare should zero everything.
    assert!(engine.prepare(44100.0, 256, 2));
    assert!(engine.buffer().iter().all(|&s| s == 0.0));
}

#[test]
fn test_lifecycle_prepare_max_block_size() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(48000.0, MAX_BLOCK_SIZE, 2));
    assert_eq!(engine.max_block_size(), MAX_BLOCK_SIZE);
}

#[test]
fn test_lifecycle_prepare_max_channels() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.prepare(48000.0, 256, MAX_CHANNELS));
    assert_eq!(engine.channels(), MAX_CHANNELS);
}

#[test]
#[should_panic(expected = "sample_rate must be > 0")]
fn test_lifecycle_prepare_zero_sample_rate_panics() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(0.0, 256, 2);
}

#[test]
#[should_panic(expected = "sample_rate must be > 0")]
fn test_lifecycle_prepare_negative_sample_rate_panics() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(-1.0, 256, 2);
}

#[test]
#[should_panic(expected = "max_block_size must be > 0")]
fn test_lifecycle_prepare_zero_block_size_panics() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 0, 2);
}

#[test]
#[should_panic(expected = "channels must be in 1..=8")]
fn test_lifecycle_prepare_zero_channels_panics() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 0);
}

#[test]
#[should_panic(expected = "channels must be in 1..=8")]
fn test_lifecycle_prepare_too_many_channels_panics() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 9);
}

#[test]
#[should_panic(expected = "max_block_size must be <= 8192")]
fn test_lifecycle_prepare_too_large_block_panics() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, MAX_BLOCK_SIZE + 1, 2);
}

// -----------------------------------------------------------------------
// Process tests
// -----------------------------------------------------------------------

#[test]
fn test_lifecycle_process_after_prepare() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64], vec![0.3; 64]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));
    assert_eq!(engine.blocks_processed(), 1);

    // With default parameters (all bands disabled by default), the output
    // should be near-zero (bypass is false, but all bands are enabled
    // with default params — gain 0dB, unity gain).
    // Actually, default BandParameters has enabled=true, gain_db=0.0,
    // which means unity gain through each band. So output ≈ input.
    for ch in 0..2 {
        for i in 0..64 {
            // Allow small numerical error from biquad processing.
            assert!(
                (output_refs[ch][i] - input_refs[ch][i]).abs() < 1e-3,
                "ch={} i={} expected {} got {} (diff {})",
                ch,
                i,
                input_refs[ch][i],
                output_refs[ch][i],
                (output_refs[ch][i] - input_refs[ch][i]).abs()
            );
        }
    }
}

#[test]
fn test_lifecycle_process_before_prepare_fails() {
    let mut engine = EngineLifecycle::new();
    let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(!engine.process(&input_refs, &mut output_refs));
    assert_eq!(engine.state(), LifecycleState::Created);
}

#[test]
fn test_lifecycle_process_block_size_1() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 1);

    let inputs: Vec<Vec<f32>> = vec![vec![0.7]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));
    // With default params (unity gain), output should be near input.
    assert!((output_refs[0][0] - 0.7).abs() < 1e-3);
}

#[test]
fn test_lifecycle_process_block_size_max() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(48000.0, MAX_BLOCK_SIZE, 2);

    let inputs: Vec<Vec<f32>> = vec![vec![1.0; MAX_BLOCK_SIZE], vec![0.5; MAX_BLOCK_SIZE]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; MAX_BLOCK_SIZE], vec![0.0; MAX_BLOCK_SIZE]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));
    assert_eq!(engine.blocks_processed(), 1);
}

#[test]
fn test_lifecycle_process_block_too_large_fails() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    let inputs: Vec<Vec<f32>> = vec![vec![0.5; 512], vec![0.3; 512]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 512], vec![0.0; 512]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    // Block size 512 > max_block_size 256 → fails.
    assert!(!engine.process(&input_refs, &mut output_refs));
}

#[test]
fn test_lifecycle_process_unequal_input_output_returns_false() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    // Real-time hot path must never panic on host channel mismatch.
    assert!(!engine.process(&input_refs, &mut output_refs));
    assert!(outputs.iter().flatten().all(|&s| s == 0.0));
}

#[test]
fn test_lifecycle_process_wrong_channel_count_fails() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    // Only 1 input for 2-channel engine.
    let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(!engine.process(&input_refs, &mut output_refs));
}

#[test]
fn test_lifecycle_process_variable_block_sizes() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    // Block size 1.
    let inputs: Vec<Vec<f32>> = vec![vec![0.1], vec![0.2]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0], vec![0.0]];
    let mut input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();
    assert!(engine.process(&input_refs, &mut output_refs));

    // Block size 64.
    let inputs: Vec<Vec<f32>> = vec![vec![0.3; 64], vec![0.4; 64]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
    input_refs = inputs.iter().map(|v| v.as_slice()).collect();
    output_refs = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();
    assert!(engine.process(&input_refs, &mut output_refs));

    // Block size 256.
    let inputs: Vec<Vec<f32>> = vec![vec![0.5; 256], vec![0.6; 256]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 256], vec![0.0; 256]];
    input_refs = inputs.iter().map(|v| v.as_slice()).collect();
    output_refs = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();
    assert!(engine.process(&input_refs, &mut output_refs));

    assert_eq!(engine.blocks_processed(), 3);
}

// -----------------------------------------------------------------------
// DSP processing tests
// -----------------------------------------------------------------------

#[test]
fn test_lifecycle_process_bypass_silences_output() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    // Set bypass to true on first process → transition starts.
    let mut params = EqParameters::default();
    params.global.bypass = true;
    engine.set_parameters(params);

    let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64], vec![0.3; 64]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));

    // First block: crossfade starts — output is a blend of dry and processed, not silence.
    // After 10ms (441 samples) of continuous bypass, output should be fully dry (equal to input).
    // For this test we just verify the engine processes without panicking and output ≈ input.
    for (ch, output_ch) in output_refs.iter().enumerate().take(2) {
        for (i, &sample) in output_ch.iter().enumerate() {
            // During transition, sample is a mix — never exactly 0.0 or input value.
            assert!(sample >= 0.0, "ch={ch} i={i} sample should be non-negative");
        }
    }

    // Process more blocks to complete the crossfade (441 samples at 44100 Hz = 10 ms).
    // After transition, bypass output should equal the input signal exactly.
    let mut outputs2: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
    let _input_refs2: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs2: Vec<&mut [f32]> = outputs2.iter_mut().map(|v| v.as_mut_slice()).collect();
    for _ in 0..10 {
        let mut outputs_tmp: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
        let input_refs_tmp: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs_tmp: Vec<&mut [f32]> =
            outputs_tmp.iter_mut().map(|v| v.as_mut_slice()).collect();
        assert!(engine.process(&input_refs_tmp, &mut output_refs_tmp));
        // Carry forward to output_refs2 for final assertion.
        outputs2 = outputs_tmp;
        output_refs2 = outputs2.iter_mut().map(|v| v.as_mut_slice()).collect();
    }

    // After transition completes, bypass output should equal input.
    for (ch, output_ch) in output_refs2.iter().enumerate().take(2) {
        for (i, &sample) in output_ch.iter().enumerate() {
            let expected = inputs[ch][i];
            assert_eq!(
                sample, expected,
                "ch={ch} i={i} post-transition bypass output should equal input (got {sample}, expected {expected})"
            );
        }
    }
}

#[test]
fn test_lifecycle_process_output_gain() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 1);

    // Set output gain to +6 dB (×2 linear).
    let mut params = EqParameters::default();
    params.global.output_gain_db = 6.0206; // ≈ ×2
    engine.set_parameters(params);

    let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));

    // Output should be ≈ input × 2.
    for i in 0..64 {
        assert!(
            (output_refs[0][i] - 1.0).abs() < 1e-2,
            "i={} expected ~1.0 got {}",
            i,
            output_refs[0][i]
        );
    }
}

#[test]
fn test_lifecycle_process_band_boost() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 1);

    // Enable band 0 with +12 dB boost at 1000 Hz.
    let mut params = EqParameters::default();
    params.bands[0] = BandParameters {
        enabled: true,
        filter_type: equaform_common::FilterType::Bell,
        frequency_hz: 1000.0,
        gain_db: 12.0,
        q: 0.707,
        slope: Slope::Db12,
    };
    engine.set_parameters(params);

    // Generate a 1000 Hz sine wave (one channel, 64 samples).
    let sample_rate = 44100.0;
    let freq = 1000.0;
    let input_samples: Vec<f32> = (0..64)
        .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin() * 0.5)
        .collect();
    let inputs: Vec<&[f32]> = vec![&input_samples];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&inputs, &mut output_refs));

    // The output should have higher amplitude than the input at 1000 Hz
    // (the bell filter boosts by +12 dB ≈ ×4 in amplitude).
    let input_rms: f32 = inputs[0].iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();
    let output_rms: f32 =
        output_refs[0].iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();

    // Output RMS should be significantly higher than input RMS.
    assert!(
        output_rms > input_rms * 1.5,
        "output RMS {output_rms} should be > input RMS {input_rms} × 1.5"
    );
}

#[test]
fn test_lifecycle_process_band_disable() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 1);

    // Disable all bands.
    let mut params = EqParameters::default();
    for band in &mut params.bands {
        band.enabled = false;
    }
    engine.set_parameters(params);

    let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));

    // With all bands disabled and no output gain, the input signal passes
    // through unchanged (no processing applied).
    for i in 0..64 {
        assert_eq!(
            output_refs[0][i], 0.5,
            "i={} output should match input (0.5) with all bands disabled, got {}",
            i, output_refs[0][i]
        );
    }
}

#[test]
fn test_lifecycle_process_stereo_channel_isolation() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    // Set different parameters for each channel by using different inputs.
    let inputs: Vec<Vec<f32>> = vec![vec![1.0; 64], vec![0.0; 64]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));

    // Channel 0 should have non-zero output (input was 1.0).
    // Channel 1 should have near-zero output (input was 0.0, and with
    // default params the biquad cascade starts from zero state).
    let ch0_sum: f32 = output_refs[0].iter().sum();
    let ch1_sum: f32 = output_refs[1].iter().sum();

    assert!(
        ch0_sum.abs() > ch1_sum.abs(),
        "ch0 sum {ch0_sum} should be > ch1 sum {ch1_sum}"
    );
}

// -----------------------------------------------------------------------
// Reset tests
// -----------------------------------------------------------------------

#[test]
fn test_lifecycle_reset() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    // Process a few blocks.
    let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64], vec![0.3; 64]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64], vec![0.0; 64]];
    for _ in 0..5 {
        let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            outputs.iter_mut().map(|v| v.as_mut_slice()).collect();
        engine.process(&input_refs, &mut output_refs);
    }
    assert_eq!(engine.blocks_processed(), 5);

    // Reset.
    engine.reset();
    assert_eq!(engine.blocks_processed(), 0);
    assert_eq!(engine.state(), LifecycleState::Ready);
    assert_eq!(engine.sample_rate(), 44100.0);
    assert_eq!(engine.channels(), 2);
    // Buffer should be zeroed.
    assert!(engine.buffer().iter().all(|&s| s == 0.0));
    // Delay line should be zeroed.
    assert!(engine.delay_line.iter().all(|&s| s == 0.0));
}

#[test]
fn test_lifecycle_reset_before_prepare() {
    let mut engine = EngineLifecycle::new();
    // Reset before prepare should be a no-op.
    engine.reset();
    assert_eq!(engine.state(), LifecycleState::Created);
    assert_eq!(engine.blocks_processed(), 0);
}

// -----------------------------------------------------------------------
// Repare tests
// -----------------------------------------------------------------------

#[test]
fn test_lifecycle_reprepare() {
    let mut engine = EngineLifecycle::new();

    // First prepare: stereo, 44100 Hz.
    assert!(engine.prepare(44100.0, 256, 2));
    assert_eq!(engine.sample_rate(), 44100.0);
    assert_eq!(engine.channels(), 2);

    // Repare: mono, 48000 Hz.
    assert!(engine.prepare(48000.0, 128, 1));
    assert_eq!(engine.sample_rate(), 48000.0);
    assert_eq!(engine.channels(), 1);
    assert_eq!(engine.state(), LifecycleState::Ready);
}

#[test]
fn test_lifecycle_reprepare_zeroes_buffer() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    // Write data to channel 0.
    engine.buffer_mut()[0] = 1.0;
    engine.buffer_mut()[1] = 2.0;

    // Repare with different params.
    assert!(engine.prepare(48000.0, 128, 1));

    // Buffer should be zeroed.
    assert!(engine.buffer().iter().all(|&s| s == 0.0));
}

// -----------------------------------------------------------------------
// Destroy tests
// -----------------------------------------------------------------------

#[test]
fn test_lifecycle_destroy() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);
    assert_eq!(engine.state(), LifecycleState::Ready);

    engine.destroy();
    assert_eq!(engine.state(), LifecycleState::Created);
    assert_eq!(engine.blocks_processed(), 0);
    // Buffer should be zeroed.
    assert!(engine.buffer().iter().all(|&s| s == 0.0));
}

#[test]
fn test_lifecycle_process_after_destroy_fails() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);
    engine.destroy();

    let inputs: Vec<Vec<f32>> = vec![vec![0.5; 64]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; 64]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(!engine.process(&input_refs, &mut output_refs));
}

#[test]
fn test_lifecycle_destroy_idempotent() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);
    engine.destroy();
    // Second destroy should be safe.
    engine.destroy();
    assert_eq!(engine.state(), LifecycleState::Created);
}

// -----------------------------------------------------------------------
// Flat buffer layout tests
// -----------------------------------------------------------------------

#[test]
fn test_flat_buffer_channel_layout() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    // Write to channel 0, sample 0.
    engine.buffer_mut()[0] = 1.0;
    // Write to channel 1, sample 0.
    engine.buffer_mut()[256] = 2.0;

    // Read back via process with matching inputs.
    let inputs: Vec<Vec<f32>> = vec![vec![1.0], vec![2.0]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0], vec![0.0]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    // Process should copy inputs to buffer, then buffer to outputs.
    assert!(engine.process(&input_refs, &mut output_refs));

    // Buffer should still have the input values.
    assert_eq!(engine.buffer()[0], 1.0);
    assert_eq!(engine.buffer()[256], 2.0);
    // Outputs should match inputs (identity pass-through with default params).
    assert_eq!(output_refs[0][0], 1.0);
    assert_eq!(output_refs[1][0], 2.0);
}

#[test]
fn test_flat_buffer_channel_isolation() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    // Write different values to each channel.
    engine.buffer_mut()[0] = 1.0;
    engine.buffer_mut()[256] = 2.0;

    // Process should preserve channel isolation.
    let inputs: Vec<Vec<f32>> = vec![vec![0.0], vec![0.0]];
    let mut outputs: Vec<Vec<f32>> = vec![vec![0.0], vec![0.0]];
    let input_refs: Vec<&[f32]> = inputs.iter().map(|v| v.as_slice()).collect();
    let mut output_refs: Vec<&mut [f32]> = outputs.iter_mut().map(|v| v.as_mut_slice()).collect();

    assert!(engine.process(&input_refs, &mut output_refs));

    // Outputs should match inputs (identity), not buffer values.
    assert_eq!(output_refs[0][0], 0.0);
    assert_eq!(output_refs[1][0], 0.0);
}

// -----------------------------------------------------------------------
// M4-T002: Split-block equivalence tests
// -----------------------------------------------------------------------

#[test]
fn test_split_block_equivalence() {
    // Process a block of 128 samples as one block vs two halves of 64.
    // Results should match within numerical tolerance.
    let mut engine1 = EngineLifecycle::new();
    engine1.prepare(44100.0, 256, 1);
    let mut params = EqParameters::default();
    params.bands[0] = BandParameters {
        enabled: true,
        filter_type: equaform_common::FilterType::Bell,
        frequency_hz: 1000.0,
        gain_db: 6.0,
        q: 0.707,
        slope: Slope::Db12,
    };
    engine1.set_parameters(params);

    let mut engine2 = EngineLifecycle::new();
    engine2.prepare(44100.0, 256, 1);
    engine2.set_parameters(params);

    // Full block of 128 samples.
    let full_input: Vec<f32> = (0..128)
        .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44100.0).sin() * 0.5)
        .collect();
    let mut full_output = vec![0.0; 128];
    let input_refs: Vec<&[f32]> = vec![&full_input];
    let mut output_refs: Vec<&mut [f32]> = vec![&mut full_output];
    assert!(engine1.process(&input_refs, &mut output_refs));

    // Split into two halves of 64.
    let half1_input: Vec<f32> = full_input[..64].to_vec();
    let mut half1_output = vec![0.0; 64];
    let input_refs1: Vec<&[f32]> = vec![&half1_input];
    let mut output_refs1: Vec<&mut [f32]> = vec![&mut half1_output];
    assert!(engine2.process(&input_refs1, &mut output_refs1));

    let half2_input: Vec<f32> = full_input[64..].to_vec();
    let mut half2_output = vec![0.0; 64];
    let input_refs2: Vec<&[f32]> = vec![&half2_input];
    let mut output_refs2: Vec<&mut [f32]> = vec![&mut half2_output];
    assert!(engine2.process(&input_refs2, &mut output_refs2));

    // Concatenate split outputs.
    let mut split_output = half1_output;
    split_output.extend(half2_output);

    // Compare: full vs split should match within tolerance.
    for i in 0..128 {
        let diff = (full_output[i] - split_output[i]).abs();
        assert!(
            diff < 1e-6,
            "split-block mismatch at sample {}: full={}, split={}, diff={}",
            i,
            full_output[i],
            split_output[i],
            diff
        );
    }
}

#[test]
fn test_split_block_equivalence_stereo() {
    // Same test for stereo: split-block should match whole-block.
    let mut engine1 = EngineLifecycle::new();
    engine1.prepare(44100.0, 256, 2);
    let mut params = EqParameters::default();
    params.bands[0] = BandParameters {
        enabled: true,
        filter_type: equaform_common::FilterType::HighPass,
        frequency_hz: 500.0,
        gain_db: 0.0,
        q: 0.707,
        slope: Slope::Db12,
    };
    engine1.set_parameters(params);

    let mut engine2 = EngineLifecycle::new();
    engine2.prepare(44100.0, 256, 2);
    engine2.set_parameters(params);

    let input: Vec<f32> = (0..128)
        .map(|i| (2.0 * std::f32::consts::PI * 800.0 * i as f32 / 44100.0).sin() * 0.3)
        .collect();
    let inputs_ch0: Vec<&[f32]> = vec![&input, &input];
    let mut out1_ch0 = vec![0.0; 128];
    let mut out1_ch1 = vec![0.0; 128];
    let mut refs1: Vec<&mut [f32]> = vec![&mut out1_ch0[..], &mut out1_ch1[..]];
    assert!(engine1.process(&inputs_ch0, &mut refs1));

    // Split: first 64 samples.
    let mut out2_ch0 = vec![0.0; 64];
    let mut out2_ch1 = vec![0.0; 64];
    let mut refs2: Vec<&mut [f32]> = vec![&mut out2_ch0[..], &mut out2_ch1[..]];
    let half_input: Vec<f32> = input[..64].to_vec();
    let half_refs: Vec<&[f32]> = vec![&half_input[..], &half_input[..]];
    assert!(engine2.process(&half_refs, &mut refs2));

    // Second 64 samples.
    let mut out3_ch0 = vec![0.0; 64];
    let mut out3_ch1 = vec![0.0; 64];
    let mut refs3: Vec<&mut [f32]> = vec![&mut out3_ch0[..], &mut out3_ch1[..]];
    let half_input2: Vec<f32> = input[64..].to_vec();
    let half_refs2: Vec<&[f32]> = vec![&half_input2[..], &half_input2[..]];
    assert!(engine2.process(&half_refs2, &mut refs3));

    // Concatenate and compare.
    let mut split_ch0 = out2_ch0;
    split_ch0.extend(out3_ch0);
    let mut split_ch1 = out2_ch1;
    split_ch1.extend(out3_ch1);

    for i in 0..128 {
        let diff0: f32 = (out1_ch0[i] - split_ch0[i]).abs();
        let diff1: f32 = (out1_ch1[i] - split_ch1[i]).abs();
        assert!(
            diff0 < 1e-6,
            "stereo ch0 split mismatch at {i}: diff={diff0}"
        );
        assert!(
            diff1 < 1e-6,
            "stereo ch1 split mismatch at {i}: diff={diff1}"
        );
    }
}

// -----------------------------------------------------------------------
// M4-T002: All eight bands coexist
// -----------------------------------------------------------------------

#[test]
fn test_all_eight_bands_coexist() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 1);

    // Enable all 8 bands with different frequencies and gains.
    let mut params = EqParameters::default();
    let band_configs = [
        (60.0, 6.0),     // Sub-bass boost
        (200.0, -3.0),   // Bass cut
        (500.0, 4.0),    // Low-mid boost
        (1000.0, -2.0),  // Mid cut
        (2500.0, 5.0),   // Upper-mid boost
        (5000.0, -4.0),  // Presence cut
        (10000.0, 3.0),  // Brilliance boost
        (14000.0, -2.0), // Air cut
    ];
    for (i, (freq, gain)) in band_configs.iter().enumerate() {
        params.bands[i] = BandParameters {
            enabled: true,
            filter_type: equaform_common::FilterType::Bell,
            frequency_hz: *freq,
            gain_db: *gain,
            q: 0.707,
            slope: Slope::Db12,
        };
    }
    engine.set_parameters(params);

    // Process a complex signal (sum of multiple frequencies).
    let input: Vec<f32> = (0..256)
        .map(|i| {
            let t = i as f32 / 44100.0;
            (2.0 * std::f32::consts::PI * 60.0 * t).sin() * 0.1
                + (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.2
                + (2.0 * std::f32::consts::PI * 5000.0 * t).sin() * 0.1
        })
        .collect();

    let mut output = vec![0.0; 256];
    let input_refs: Vec<&[f32]> = vec![&input];
    let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
    assert!(engine.process(&input_refs, &mut output_refs));

    // Output should be non-zero (processing happened).
    let output_rms: f32 = output.iter().map(|s| s * s).sum::<f32>().sqrt() / 256.0_f32.sqrt();
    assert!(
        output_rms > 0.01,
        "all-8-bands output RMS {output_rms} should be > 0.01"
    );
}

// -----------------------------------------------------------------------
// M4-T002: Block size variety tests
// -----------------------------------------------------------------------

#[test]
fn test_block_sizes_2_3_4_8() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 1);

    let mut params = EqParameters::default();
    params.bands[0] = BandParameters {
        enabled: true,
        filter_type: equaform_common::FilterType::LowPass,
        frequency_hz: 2000.0,
        gain_db: 0.0,
        q: 0.707,
        slope: Slope::Db12,
    };
    engine.set_parameters(params);

    for block_size in [2, 3, 4, 8] {
        let input: Vec<f32> = (0..block_size)
            .map(|i| (2.0 * std::f32::consts::PI * 500.0 * i as f32 / 44100.0).sin() * 0.5)
            .collect();
        let mut output = vec![0.0; block_size];
        let input_refs: Vec<&[f32]> = vec![&input];
        let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
        assert!(engine.process(&input_refs, &mut output_refs));

        // Output should be non-zero.
        let sum: f32 = output.iter().map(|s| s.abs()).sum();
        assert!(
            sum > 0.0,
            "block_size={block_size} output should be non-zero"
        );
    }
}

#[test]
fn test_block_size_powers_of_two() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 8192, 1);

    let input: Vec<f32> = (0..8192)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin() * 0.5)
        .collect();
    let mut output = vec![0.0; 8192];
    let input_refs: Vec<&[f32]> = vec![&input];
    let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
    assert!(engine.process(&input_refs, &mut output_refs));
    assert_eq!(engine.blocks_processed(), 1);
}

#[test]
fn test_silence_input() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 1);

    let mut params = EqParameters::default();
    params.bands[0] = BandParameters {
        enabled: true,
        filter_type: equaform_common::FilterType::Bell,
        frequency_hz: 1000.0,
        gain_db: 12.0,
        q: 0.707,
        slope: Slope::Db12,
    };
    engine.set_parameters(params);

    let input: Vec<f32> = vec![0.0; 64];
    let mut output = vec![0.0; 64];
    let input_refs: Vec<&[f32]> = vec![&input];
    let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
    assert!(engine.process(&input_refs, &mut output_refs));

    // Silence in → silence out (no stored state after reset).
    let sum: f32 = output.iter().map(|s| s.abs()).sum();
    assert!(
        sum < 1e-10,
        "silence input should produce silence output, got sum={sum}"
    );
}

#[test]
fn test_impulse_response() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 1);

    let mut params = EqParameters::default();
    params.bands[0] = BandParameters {
        enabled: true,
        filter_type: equaform_common::FilterType::Bell,
        frequency_hz: 1000.0,
        gain_db: 6.0,
        q: 2.0, // High Q for ringing
        slope: Slope::Db12,
    };
    engine.set_parameters(params);

    // Impulse at sample 0.
    let mut input = vec![1.0; 64];
    for item in &mut input[1..] {
        *item = 0.0;
    }
    let mut output = vec![0.0; 64];
    let input_refs: Vec<&[f32]> = vec![&input];
    let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
    assert!(engine.process(&input_refs, &mut output_refs));

    // Impulse should produce non-zero output (filter response).
    let sum: f32 = output.iter().map(|s| s.abs()).sum();
    assert!(sum > 0.5, "impulse response sum {sum} should be > 0.5");
}

// -----------------------------------------------------------------------
// M4-T002: Channel routing correctness
// -----------------------------------------------------------------------

#[test]
fn test_channel_routing_mono() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 1);

    let input: Vec<f32> = (0..64)
        .map(|i| (2.0 * std::f32::consts::PI * 800.0 * i as f32 / 44100.0).sin() * 0.7)
        .collect();
    let mut output = vec![0.0; 64];
    let input_refs: Vec<&[f32]> = vec![&input];
    let mut output_refs: Vec<&mut [f32]> = vec![&mut output];
    assert!(engine.process(&input_refs, &mut output_refs));

    // Output should match input pattern (unity gain, default params).
    for (i, (out_s, in_s)) in output_refs[0].iter().zip(&input).enumerate() {
        assert!(
            (out_s - in_s).abs() < 1e-3,
            "mono ch0 sample {i}: expected {in_s} got {out_s}"
        );
    }
}

#[test]
fn test_channel_routing_stereo_different_content() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    // Different content per channel.
    let ch0: Vec<f32> = (0..64).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
    let ch1: Vec<f32> = (0..64).map(|i| (i as f32 * 0.02).sin() * 0.3).collect();
    let inputs: Vec<&[f32]> = vec![&ch0, &ch1];
    let mut out0 = vec![0.0; 64];
    let mut out1 = vec![0.0; 64];
    let mut output_refs: Vec<&mut [f32]> = vec![&mut out0, &mut out1];
    assert!(engine.process(&inputs, &mut output_refs));

    // Each channel should preserve its own content (unity gain).
    for (i, (out_s, ch_s)) in out0.iter().zip(&ch0).enumerate() {
        let diff = (out_s - ch_s).abs();
        assert!(diff < 1e-3, "stereo ch0 sample {i}: diff={diff}");
    }
    for (i, (out_s, ch_s)) in out1.iter().zip(&ch1).enumerate() {
        let diff = (out_s - ch_s).abs();
        assert!(diff < 1e-3, "stereo ch1 sample {i}: diff={diff}");
    }
}

#[test]
fn test_channel_routing_stereo_with_filter() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(44100.0, 256, 2);

    // Boost 1000 Hz on both channels.
    let mut params = EqParameters::default();
    params.bands[0] = BandParameters {
        enabled: true,
        filter_type: equaform_common::FilterType::Bell,
        frequency_hz: 1000.0,
        gain_db: 12.0,
        q: 0.707,
        slope: Slope::Db12,
    };
    engine.set_parameters(params);

    let ch0: Vec<f32> = (0..64)
        .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44100.0).sin() * 0.5)
        .collect();
    let ch1: Vec<f32> = (0..64)
        .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44100.0).sin() * 0.3)
        .collect();
    let inputs: Vec<&[f32]> = vec![&ch0, &ch1];
    let mut out0 = vec![0.0; 64];
    let mut out1 = vec![0.0; 64];
    let mut output_refs: Vec<&mut [f32]> = vec![&mut out0, &mut out1];
    assert!(engine.process(&inputs, &mut output_refs));

    // Both channels should be boosted (output amplitude > input amplitude).
    let ch0_in_rms: f32 = ch0.iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();
    let ch0_out_rms: f32 = out0.iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();
    let ch1_in_rms: f32 = ch1.iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();
    let ch1_out_rms: f32 = out1.iter().map(|s| s * s).sum::<f32>().sqrt() / 64.0_f32.sqrt();

    assert!(
        ch0_out_rms > ch0_in_rms * 1.5,
        "ch0 boost: in_rms={ch0_in_rms}, out_rms={ch0_out_rms}"
    );
    assert!(
        ch1_out_rms > ch1_in_rms * 1.5,
        "ch1 boost: in_rms={ch1_in_rms}, out_rms={ch1_out_rms}"
    );
}

// -----------------------------------------------------------------------
// M4-T002: Split-block equivalence with filter
// -----------------------------------------------------------------------

#[test]
fn test_split_block_with_filter() {
    // Verify split-block equivalence when filters are active.
    let mut engine1 = EngineLifecycle::new();
    engine1.prepare(44100.0, 256, 1);
    let mut params = EqParameters::default();
    params.bands[0] = BandParameters {
        enabled: true,
        filter_type: equaform_common::FilterType::LowPass,
        frequency_hz: 2000.0,
        gain_db: 0.0,
        q: 0.707,
        slope: Slope::Db12,
    };
    params.bands[1] = BandParameters {
        enabled: true,
        filter_type: equaform_common::FilterType::Bell,
        frequency_hz: 500.0,
        gain_db: 6.0,
        q: 0.707,
        slope: Slope::Db12,
    };
    engine1.set_parameters(params);

    let mut engine2 = EngineLifecycle::new();
    engine2.prepare(44100.0, 256, 1);
    engine2.set_parameters(params);

    let input: Vec<f32> = (0..128)
        .map(|i| {
            (2.0 * std::f32::consts::PI * 300.0 * i as f32 / 44100.0).sin() * 0.3
                + (2.0 * std::f32::consts::PI * 1500.0 * i as f32 / 44100.0).sin() * 0.2
        })
        .collect();

    // Full block.
    let mut full_output = vec![0.0; 128];
    let input_refs: Vec<&[f32]> = vec![&input];
    let mut output_refs: Vec<&mut [f32]> = vec![&mut full_output];
    assert!(engine1.process(&input_refs, &mut output_refs));

    // Split into two halves.
    let mut split_output = vec![0.0; 128];
    let half1 = &input[..64];
    let half2 = &input[64..];
    let mut half1_out = vec![0.0; 64];
    let mut half2_out = vec![0.0; 64];
    let refs1: Vec<&[f32]> = vec![half1];
    let mut refs2: Vec<&mut [f32]> = vec![&mut half1_out];
    assert!(engine2.process(&refs1, &mut refs2));
    let refs3: Vec<&[f32]> = vec![half2];
    let mut refs4: Vec<&mut [f32]> = vec![&mut half2_out];
    assert!(engine2.process(&refs3, &mut refs4));
    split_output[..64].copy_from_slice(&half1_out);
    split_output[64..].copy_from_slice(&half2_out);

    // Compare.
    for i in 0..128 {
        let diff = (full_output[i] - split_output[i]).abs();
        assert!(
            diff < 1e-6,
            "split-block with filter mismatch at {}: full={}, split={}, diff={}",
            i,
            full_output[i],
            split_output[i],
            diff
        );
    }
}

#[test]
fn test_coefficient_caching_and_dirty_flag() {
    let mut engine = EngineLifecycle::new();
    assert!(engine.coeff_cache.dirty);

    engine.prepare(48000.0, 256, 2);
    assert!(!engine.coeff_cache.dirty);

    let initial_coeffs = engine.coeff_cache.coeffs[0];
    assert!(initial_coeffs.is_finite());

    // Process a block without parameter change - remains clean
    let input_l = [0.1f32; 64];
    let input_r = [0.1f32; 64];
    let mut out_l = [0.0f32; 64];
    let mut out_r = [0.0f32; 64];
    assert!(engine.process(&[&input_l, &input_r], &mut [&mut out_l, &mut out_r]));
    assert!(!engine.coeff_cache.dirty);
    assert_eq!(engine.coeff_cache.coeffs[0], initial_coeffs);

    // Change parameters - flags dirty
    let mut params = *engine.eq_parameters();
    params.bands[0].gain_db = 6.0;
    engine.set_parameters(params);
    assert!(engine.coeff_cache.dirty);

    // Next process - recomputes and clears dirty flag
    assert!(engine.process(&[&input_l, &input_r], &mut [&mut out_l, &mut out_r]));
    assert!(!engine.coeff_cache.dirty);
    assert_ne!(engine.coeff_cache.coeffs[0], initial_coeffs);
}

#[test]
fn test_bidirectional_bypass_crossfade() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(48000.0, 256, 1);

    // Boost 1kHz by +12dB
    let mut params = *engine.eq_parameters();
    params.bands[0].enabled = true;
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 12.0;
    params.bands[0].q = 1.0;
    params.global.bypass = false;
    engine.set_parameters(params);

    // 1kHz sine wave at 48kHz
    let pi2 = std::f32::consts::PI * 2.0;
    let mut input = vec![0.0f32; 128];
    for (i, sample) in input.iter_mut().enumerate() {
        *sample = (pi2 * 1000.0 * i as f32 / 48000.0).sin();
    }
    let mut out = vec![0.0f32; 128];

    // 1. Let filter settle in active state
    for _ in 0..15 {
        assert!(engine.process(&[&input], &mut [&mut out]));
    }
    let active_peak = out.iter().cloned().fold(0.0f32, f32::max);
    // +12dB should boost sine amplitude from 1.0 to ~3.98
    assert!(
        active_peak > 3.5,
        "active peak should be ~3.98, got {active_peak}"
    );

    // 2. Engage bypass (active -> bypass)
    params.global.bypass = true;
    engine.set_parameters(params);

    // Crossfade for 20ms at 48kHz = 960 samples (10 blocks of 128 = 1280 samples)
    for _ in 0..10 {
        assert!(engine.process(&[&input], &mut [&mut out]));
    }
    // At full bypass, peak should match dry sine amplitude (1.0)
    let bypassed_peak = out.iter().cloned().fold(0.0f32, f32::max);
    assert!(
        (bypassed_peak - 1.0).abs() < 0.05,
        "bypassed peak should match dry 1.0, got {bypassed_peak}"
    );

    // 3. Disengage bypass (bypass -> active)
    params.global.bypass = false;
    engine.set_parameters(params);

    for _ in 0..15 {
        assert!(engine.process(&[&input], &mut [&mut out]));
    }
    let re_active_peak = out.iter().cloned().fold(0.0f32, f32::max);
    assert!(
        (re_active_peak - active_peak).abs() < 0.05,
        "re-activated peak should match active peak {active_peak}, got {re_active_peak}"
    );
}

#[test]
fn test_auto_gain_compensation() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(48000.0, 128, 1);

    let mut params = EqParameters::default();
    params.global.bypass = false;
    params.global.auto_gain = false;
    // Boost 1000 Hz by +12 dB
    params.bands[0].enabled = true;
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 12.0;
    params.bands[0].filter_type = FilterType::Bell;
    params.bands[0].q = 1.0;
    engine.set_parameters(params);

    let pi2 = std::f32::consts::PI * 2.0;
    let mut input = vec![0.0f32; 128];
    for (i, sample) in input.iter_mut().enumerate() {
        *sample = (pi2 * 1000.0 * i as f32 / 48000.0).sin();
    }
    let mut out = vec![0.0f32; 128];

    // Process without auto_gain
    for _ in 0..15 {
        assert!(engine.process(&[&input], &mut [&mut out]));
    }
    let peak_without_auto_gain = out.iter().cloned().fold(0.0f32, f32::max);
    assert!(peak_without_auto_gain > 3.5);

    // Enable auto_gain
    params.global.auto_gain = true;
    engine.set_parameters(params);

    for _ in 0..15 {
        assert!(engine.process(&[&input], &mut [&mut out]));
    }
    let peak_with_auto_gain = out.iter().cloned().fold(0.0f32, f32::max);

    // Auto gain should reduce the output amplitude
    assert!(
        peak_with_auto_gain < peak_without_auto_gain * 0.8,
        "auto_gain should reduce peak, got {peak_with_auto_gain} vs {peak_without_auto_gain}"
    );
    let comp_db = engine.auto_gain_compensation_db();
    assert!(
        comp_db < 0.0,
        "comp_db should be negative for a boost, got {comp_db}"
    );
}

#[test]
fn test_automation_persistence_across_blocks() {
    use equaform_common::automation::AutomationEvent;

    let mut engine = EngineLifecycle::new();
    engine.prepare(48000.0, 128, 1);

    let mut params = EqParameters::default();
    params.bands[0].enabled = true;
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 0.0;
    params.bands[0].filter_type = FilterType::Bell;
    params.bands[0].q = 1.0;
    engine.set_parameters(params);

    let gain_param_id = params.bands[0].gain_param_id();

    // Enqueue an automation event boosting gain to normalized value 1.0 (+24 dB) at offset 0
    let event = AutomationEvent::new(gain_param_id, 1.0, 0, 1);
    engine.push_automation_event(event);

    let pi2 = std::f32::consts::PI * 2.0;
    let mut input = vec![0.0f32; 128];
    for (i, sample) in input.iter_mut().enumerate() {
        *sample = (pi2 * 1000.0 * i as f32 / 48000.0).sin();
    }
    let mut out1 = vec![0.0f32; 128];
    let mut out2 = vec![0.0f32; 128];

    // Process block 1 with automation
    assert!(engine.process(&[&input], &mut [&mut out1]));
    assert_eq!(engine.eq_parameters().bands[0].gain_db, 24.0);

    // Process block 2 without any new automation events
    assert!(engine.process(&[&input], &mut [&mut out2]));
    // State must persist!
    assert_eq!(engine.eq_parameters().bands[0].gain_db, 24.0);

    // Crucial DSP check: Verify that block 2 audio remains boosted (+24 dB = factor ~15.8)
    let peak_block2 = out2.iter().cloned().fold(0.0f32, f32::max);
    assert!(
        peak_block2 > 8.0,
        "Block 2 audio must retain biquad +24 dB gain boost across blocks, got peak {peak_block2}"
    );
}

#[test]
fn test_rapid_bypass_toggles_mid_crossfade_no_glitch() {
    let mut engine = EngineLifecycle::new();
    engine.prepare(48000.0, 64, 1);

    // Apply a strong +12 dB output gain so wet and dry levels differ significantly.
    let mut params = EqParameters::default();
    params.global.output_gain_db = 12.0;
    params.global.bypass = false;
    engine.set_parameters(params);

    // Constant DC input makes any amplitude step discontinuity immediately visible.
    let input = [0.25_f32; 64];
    let mut out = [0.0_f32; 64];

    // Warm up first block (active).
    assert!(engine.process(&[&input], &mut [&mut out]));
    let mut prev_sample = out[63];

    // Rapidly toggle bypass every 64 samples (~1.33 ms, well inside the 5 ms crossfade).
    let mut max_delta = 0.0_f32;
    for toggle_idx in 0..10 {
        params.global.bypass = toggle_idx % 2 == 0;
        engine.set_parameters(params);
        assert!(engine.process(&[&input], &mut [&mut out]));

        for &sample in &out {
            let delta = (sample - prev_sample).abs();
            if delta > max_delta {
                max_delta = delta;
            }
            prev_sample = sample;
        }
    }

    // With 5 ms crossfade at 48 kHz (240 samples) and wet-dry diff ~0.75, max
    // continuous sample-to-sample step is well under 0.02 (a reset glitch jumps > 0.3).
    assert!(
        max_delta < 0.05,
        "Rapid mid-crossfade bypass toggle caused a step discontinuity: max_delta={max_delta}"
    );
}

// ===========================================================================
// Band Solo audition tests
// ===========================================================================

fn solo_test_sine(freq_hz: f32, sr: f32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sr).sin())
        .collect()
}

fn solo_test_rms(samples: &[f32]) -> f32 {
    let mean_sq = samples.iter().map(|&s| s * s).sum::<f32>() / samples.len().max(1) as f32;
    mean_sq.sqrt()
}

fn run_stereo_blocks(engine: &mut EngineLifecycle, signal: &[f32], block_size: usize) -> Vec<f32> {
    let mut last_block = vec![0.0f32; block_size];
    let mut out_r = vec![0.0f32; block_size];
    for chunk in signal.chunks_exact(block_size) {
        assert!(engine.process(&[chunk, chunk], &mut [&mut last_block, &mut out_r]));
    }
    last_block
}

#[test]
fn test_band_solo_isolates_center_frequency() {
    let mut params = EqParameters::default();
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].q = 2.0;
    params.set_band_solo(0, true);

    let sig_1k = solo_test_sine(1000.0, 48000.0, 256 * 16);
    let sig_100 = solo_test_sine(100.0, 48000.0, 256 * 16);

    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(48000.0, 256, 2);
    engine.set_parameters(params);
    let out_1k = run_stereo_blocks(&mut engine, &sig_1k, 256);

    engine.reset();
    let out_100 = run_stereo_blocks(&mut engine, &sig_100, 256);

    let in_rms = solo_test_rms(&sig_1k[..256]);
    assert!(solo_test_rms(&out_1k) > 0.7 * in_rms);
    assert!(solo_test_rms(&out_100) < 0.25 * in_rms);
}

#[test]
fn test_multi_band_solo_sums_in_parallel() {
    let mut params = EqParameters::default();
    params.bands[0].frequency_hz = 200.0;
    params.bands[0].q = 2.0;
    params.bands[1].frequency_hz = 4000.0;
    params.bands[1].q = 2.0;
    params.set_band_solo(0, true);
    params.set_band_solo(1, true);

    let sig_200 = solo_test_sine(200.0, 48000.0, 256 * 16);
    let sig_4k = solo_test_sine(4000.0, 48000.0, 256 * 16);

    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(48000.0, 256, 2);
    engine.set_parameters(params);
    let out_200 = run_stereo_blocks(&mut engine, &sig_200, 256);

    engine.reset();
    let out_4k = run_stereo_blocks(&mut engine, &sig_4k, 256);

    assert!(solo_test_rms(&out_200) > 0.6 * solo_test_rms(&sig_200[..256]));
    assert!(solo_test_rms(&out_4k) > 0.6 * solo_test_rms(&sig_4k[..256]));
}

#[test]
fn test_bypass_overrides_band_solo() {
    let mut params = EqParameters::default();
    params.bands[0].frequency_hz = 4000.0;
    params.bands[0].q = 2.0;
    params.set_band_solo(0, true);
    params.global.bypass = true;

    let sig = solo_test_sine(100.0, 48000.0, 256 * 16);
    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(48000.0, 256, 2);
    engine.set_parameters(params);
    let out = run_stereo_blocks(&mut engine, &sig, 256);

    let last_in = &sig[sig.len() - 256..];
    for (&o, &i) in out.iter().zip(last_in.iter()) {
        assert!(
            (o - i).abs() < 1e-4,
            "Bypass must pass dry signal unmodified"
        );
    }
}

#[test]
fn test_solo_clear_restores_normal_eq() {
    let mut params = EqParameters::default();
    params.bands[0].frequency_hz = 4000.0;
    params.bands[0].gain_db = 0.0;
    params.bands[0].q = 2.0;
    params.set_band_solo(0, true);

    let sig = solo_test_sine(100.0, 48000.0, 256 * 16);
    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(48000.0, 256, 2);
    engine.set_parameters(params);
    let solo_out = run_stereo_blocks(&mut engine, &sig, 256);
    let in_rms = solo_test_rms(&sig[..256]);
    assert!(solo_test_rms(&solo_out) < 0.25 * in_rms);

    params.clear_solo();
    engine.set_parameters(params);
    let restored_out = run_stereo_blocks(&mut engine, &sig, 256);
    assert!(solo_test_rms(&restored_out) > 0.95 * in_rms);
}

// ===========================================================================
// Mid/Side & per-band stereo routing tests
// ===========================================================================

#[test]
fn test_ms_side_band_does_not_affect_pure_mono() {
    let mut eng = Box::new(EngineLifecycle::new());
    eng.prepare(48000.0, 256, 2);
    let mut params = EqParameters::default();
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 12.0;
    params.set_band_routing(0, equaform_common::BandRouting::Side);
    eng.set_parameters(params);

    let sig = solo_test_sine(1000.0, 48000.0, 16 * 256);
    let mut ol = vec![0.0f32; 256];
    let mut or = vec![0.0f32; 256];
    for chunk in sig.chunks_exact(256) {
        assert!(eng.process(&[chunk, chunk], &mut [&mut ol, &mut or]));
        for ((&l_out, &r_out), &in_s) in ol.iter().zip(or.iter()).zip(chunk.iter()) {
            assert!((l_out - in_s).abs() < 1e-4);
            assert!((r_out - in_s).abs() < 1e-4);
        }
    }
}

#[test]
fn test_ms_mid_band_boosts_mono_and_leaves_antiphase_unmodified() {
    let mut eng = Box::new(EngineLifecycle::new());
    eng.prepare(48000.0, 256, 2);
    let mut params = EqParameters::default();
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 12.0;
    params.set_band_routing(0, equaform_common::BandRouting::Mid);
    eng.set_parameters(params);

    let sig = solo_test_sine(1000.0, 48000.0, 16 * 256);
    let neg_sig: Vec<f32> = sig.iter().map(|&x| -x).collect();
    let mut ol = vec![0.0f32; 256];
    let mut or = vec![0.0f32; 256];

    // Pure anti-phase (L = +s, R = -s): Mid == 0, output must match input.
    for (c_l, c_r) in sig.chunks_exact(256).zip(neg_sig.chunks_exact(256)) {
        assert!(eng.process(&[c_l, c_r], &mut [&mut ol, &mut or]));
        for ((&l_out, &r_out), &in_s) in ol.iter().zip(or.iter()).zip(c_l.iter()) {
            assert!((l_out - in_s).abs() < 1e-4);
            assert!((r_out + in_s).abs() < 1e-4);
        }
    }

    // Pure mono (L == R): Mid carries full energy, +12 dB boost yields > 3x RMS.
    eng.reset();
    let out_mono = run_stereo_blocks(&mut eng, &sig, 256);
    assert!(solo_test_rms(&out_mono) > 3.0 * solo_test_rms(&sig[..256]));
}

#[test]
fn test_left_only_band_isolates_right_channel() {
    let mut eng = Box::new(EngineLifecycle::new());
    eng.prepare(48000.0, 256, 2);
    let mut params = EqParameters::default();
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 12.0;
    params.set_band_routing(0, equaform_common::BandRouting::Left);
    eng.set_parameters(params);

    let sig = solo_test_sine(1000.0, 48000.0, 16 * 256);
    let mut ol = vec![0.0f32; 256];
    let mut or = vec![0.0f32; 256];
    for chunk in sig.chunks_exact(256) {
        assert!(eng.process(&[chunk, chunk], &mut [&mut ol, &mut or]));
        for (&r_out, &in_s) in or.iter().zip(chunk.iter()) {
            assert!((r_out - in_s).abs() < 1e-4);
        }
    }
    assert!(solo_test_rms(&ol) > 3.0 * solo_test_rms(&sig[..256]));
}

#[test]
fn test_solo_toggle_is_click_free() {
    let mut eng = Box::new(EngineLifecycle::new());
    eng.prepare(48_000.0, 64, 2);

    let mut params = EqParameters::default();
    params.bands[0].frequency_hz = 100.0;
    params.bands[0].q = 2.0;
    eng.set_parameters(params);

    let sr = 48_000.0;
    let block = 64usize;
    let total_blocks = 30usize;
    let mut out_l_all = Vec::with_capacity(total_blocks * block);
    let mut out_r_all = Vec::with_capacity(total_blocks * block);

    let mut in_block = vec![0.0_f32; block];
    let mut out_l = vec![0.0_f32; block];
    let mut out_r = vec![0.0_f32; block];

    for b in 0..total_blocks {
        if b == 10 {
            params.set_band_solo(0, true);
            eng.set_parameters(params);
        } else if b == 20 {
            params.set_band_solo(0, false);
            eng.set_parameters(params);
        }

        for (i, sample) in in_block.iter_mut().enumerate() {
            let t = (b * block + i) as f32 / sr;
            *sample = 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
        }

        assert!(eng.process(&[&in_block, &in_block], &mut [&mut out_l, &mut out_r]));
        out_l_all.extend_from_slice(&out_l);
        out_r_all.extend_from_slice(&out_r);
    }

    let max_step_l = out_l_all
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0_f32, f32::max);
    let max_step_r = out_r_all
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0_f32, f32::max);

    assert!(
        max_step_l <= 0.20 && max_step_r <= 0.20,
        "solo toggle produced discontinuity: max_step_l={max_step_l}, max_step_r={max_step_r}"
    );
}

#[test]
fn test_solo_mixed_mid_side_and_left_superposition() {
    let sr = 48_000.0;
    let block = 256usize;
    let in_l = solo_test_sine(500.0, sr, block);
    let in_r = solo_test_sine(1500.0, sr, block);

    let run_solo = |setup: fn(&mut EqParameters)| -> (Vec<f32>, Vec<f32>) {
        let mut eng = Box::new(EngineLifecycle::new());
        eng.prepare(f64::from(sr), block, 2);
        let mut params = EqParameters::default();
        params.bands[0].frequency_hz = 500.0;
        params.bands[1].frequency_hz = 1500.0;
        params.bands[2].frequency_hz = 1000.0;
        params.set_band_routing(0, equaform_common::BandRouting::Mid);
        params.set_band_routing(1, equaform_common::BandRouting::Side);
        params.set_band_routing(2, equaform_common::BandRouting::Left);
        setup(&mut params);
        eng.set_parameters(params);
        let mut out_l = vec![0.0_f32; block];
        let mut out_r = vec![0.0_f32; block];
        assert!(eng.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
        (out_l, out_r)
    };

    let (mid_l, mid_r) = run_solo(|p| p.set_band_solo(0, true));
    let (side_l, side_r) = run_solo(|p| p.set_band_solo(1, true));
    let (left_l, left_r) = run_solo(|p| p.set_band_solo(2, true));
    let (all_l, all_r) = run_solo(|p| {
        p.set_band_solo(0, true);
        p.set_band_solo(1, true);
        p.set_band_solo(2, true);
    });

    for i in 0..block {
        let exp_l = mid_l[i] + side_l[i] + left_l[i];
        let exp_r = mid_r[i] + side_r[i] + left_r[i];
        assert!(
            (all_l[i] - exp_l).abs() < 1e-5,
            "sample {i} L mismatch: got {}, expected {exp_l}",
            all_l[i]
        );
        assert!(
            (all_r[i] - exp_r).abs() < 1e-5,
            "sample {i} R mismatch: got {}, expected {exp_r}",
            all_r[i]
        );
    }
}

// ===========================================================================
// Linear-phase convolver, dry delay line, and mode transition tests
// ===========================================================================

#[test]
fn test_latency_samples_by_processing_mode() {
    use equaform_common::ProcessingMode;

    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(48_000.0, 256, 2);

    assert_eq!(engine.latency_samples(), 0);

    let mut params = *engine.eq_parameters();
    params.global.processing_mode = ProcessingMode::LinearPhaseLow;
    engine.set_parameters(params);
    assert_eq!(engine.latency_samples(), 1024);

    params.global.processing_mode = ProcessingMode::LinearPhaseHigh;
    engine.set_parameters(params);
    assert_eq!(engine.latency_samples(), 2048);

    params.global.processing_mode = ProcessingMode::MinimumPhase;
    engine.set_parameters(params);
    assert_eq!(engine.latency_samples(), 0);
}

#[test]
fn test_linear_phase_impulse_peaks_at_exact_latency() {
    use equaform_common::ProcessingMode;

    for (mode, expected_peak, total_len) in [
        (ProcessingMode::LinearPhaseLow, 1024_usize, 4096_usize),
        (ProcessingMode::LinearPhaseHigh, 2048_usize, 6144_usize),
    ] {
        let mut engine = Box::new(EngineLifecycle::new());
        engine.prepare(48_000.0, 256, 2);

        let mut params = EqParameters::default();
        params.global.processing_mode = mode;
        engine.set_parameters(params);

        let mut in_l = vec![0.0_f32; total_len];
        let mut in_r = vec![0.0_f32; total_len];
        in_l[0] = 1.0;
        in_r[0] = 1.0;

        let mut out_l = Vec::with_capacity(total_len);
        let mut out_r = Vec::with_capacity(total_len);
        let mut blk_l = [0.0_f32; 256];
        let mut blk_r = [0.0_f32; 256];

        for (cl, cr) in in_l.chunks_exact(256).zip(in_r.chunks_exact(256)) {
            assert!(engine.process(&[cl, cr], &mut [&mut blk_l, &mut blk_r]));
            out_l.extend_from_slice(&blk_l);
            out_r.extend_from_slice(&blk_r);
        }

        let peak_l = out_l
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let peak_r = out_r
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .map(|(i, _)| i)
            .unwrap_or(0);

        assert_eq!(peak_l, expected_peak, "mode={mode:?} L peak mismatch");
        assert_eq!(peak_r, expected_peak, "mode={mode:?} R peak mismatch");
        assert!((out_l[expected_peak] - 1.0).abs() < 1e-4);
        assert!((out_r[expected_peak] - 1.0).abs() < 1e-4);
    }
}

#[test]
fn test_linear_phase_bypass_delays_dry_by_exact_latency() {
    use equaform_common::ProcessingMode;

    for (mode, expected_delay, total_len) in [
        (ProcessingMode::LinearPhaseLow, 1024_usize, 3072_usize),
        (ProcessingMode::LinearPhaseHigh, 2048_usize, 4096_usize),
    ] {
        let mut engine = Box::new(EngineLifecycle::new());
        engine.prepare(48_000.0, 256, 2);

        let mut params = EqParameters::default();
        params.global.processing_mode = mode;
        params.global.bypass = true;
        params.bands[0].enabled = true;
        params.bands[0].gain_db = 12.0;
        engine.set_parameters(params);

        let mut in_l = vec![0.0_f32; total_len];
        let mut in_r = vec![0.0_f32; total_len];
        in_l[0] = 1.0;
        in_r[0] = 0.5;

        let mut out_l = Vec::with_capacity(total_len);
        let mut out_r = Vec::with_capacity(total_len);
        let mut blk_l = [0.0_f32; 256];
        let mut blk_r = [0.0_f32; 256];

        for (cl, cr) in in_l.chunks_exact(256).zip(in_r.chunks_exact(256)) {
            assert!(engine.process(&[cl, cr], &mut [&mut blk_l, &mut blk_r]));
            out_l.extend_from_slice(&blk_l);
            out_r.extend_from_slice(&blk_r);
        }

        for i in 0..total_len {
            let exp_l = if i == expected_delay { 1.0 } else { 0.0 };
            let exp_r = if i == expected_delay { 0.5 } else { 0.0 };
            assert!(
                (out_l[i] - exp_l).abs() < 1e-6 && (out_r[i] - exp_r).abs() < 1e-6,
                "mode={mode:?} sample {i}: got ({}, {}), expected ({exp_l}, {exp_r})",
                out_l[i],
                out_r[i]
            );
        }
    }
}

#[test]
fn test_linear_phase_bypass_toggle_mid_stream_is_phase_aligned() {
    use equaform_common::ProcessingMode;

    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(48_000.0, 256, 2);

    let mut params = EqParameters::default();
    params.global.processing_mode = ProcessingMode::LinearPhaseLow;
    params.global.bypass = false;
    engine.set_parameters(params);

    let sr = 48_000.0_f32;
    let block = 256_usize;
    let mut out_l = vec![0.0_f32; block];
    let mut out_r = vec![0.0_f32; block];

    // Feed 16 blocks (4096 samples) of flat active EQ so both convolver and DryDelayLine settle.
    let mut sample_idx = 0_usize;
    for _ in 0..16 {
        let sig: Vec<f32> = (0..block)
            .map(|i| {
                0.5 * (2.0 * std::f32::consts::PI * 500.0 * (sample_idx + i) as f32 / sr).sin()
            })
            .collect();
        sample_idx += block;
        assert!(engine.process(&[&sig, &sig], &mut [&mut out_l, &mut out_r]));
    }
    let last_active = out_l[block - 1];

    // Toggle bypass ON: DryDelayLine must already be populated and phase-aligned.
    params.global.bypass = true;
    engine.set_parameters(params);

    let mut max_step = 0.0_f32;
    let mut prev = last_active;
    for _ in 0..8 {
        let sig: Vec<f32> = (0..block)
            .map(|i| {
                0.5 * (2.0 * std::f32::consts::PI * 500.0 * (sample_idx + i) as f32 / sr).sin()
            })
            .collect();
        sample_idx += block;
        assert!(engine.process(&[&sig, &sig], &mut [&mut out_l, &mut out_r]));
        for &s in &out_l {
            assert!(s.is_finite());
            max_step = max_step.max((s - prev).abs());
            prev = s;
        }
    }

    assert!(
        max_step < 0.08,
        "phase-aligned bypass toggle jumped: max_step={max_step}"
    );
}

#[test]
fn test_mode_switching_cycle_is_finite_and_click_free() {
    use equaform_common::ProcessingMode;

    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(48_000.0, 64, 2);

    let mut params = EqParameters::default();
    params.bands[0].enabled = true;
    params.bands[0].filter_type = FilterType::Bell;
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 6.0;
    params.bands[0].q = 1.0;
    params.global.processing_mode = ProcessingMode::MinimumPhase;
    engine.set_parameters(params);

    let sr = 48_000.0_f32;
    let block = 64_usize;
    let mut out_l_all = Vec::new();
    let mut out_r_all = Vec::new();
    let mut in_blk = vec![0.0_f32; block];
    let mut out_l = vec![0.0_f32; block];
    let mut out_r = vec![0.0_f32; block];

    let stages = [
        (ProcessingMode::MinimumPhase, 40_usize),
        (ProcessingMode::LinearPhaseLow, 60_usize),
        (ProcessingMode::LinearPhaseHigh, 80_usize),
        (ProcessingMode::MinimumPhase, 40_usize),
    ];

    let mut sample_cursor = 0_usize;
    for (mode, num_blocks) in stages {
        params.global.processing_mode = mode;
        engine.set_parameters(params);
        for _ in 0..num_blocks {
            for (i, s) in in_blk.iter_mut().enumerate() {
                let t = (sample_cursor + i) as f32 / sr;
                *s = 0.25 * (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
            }
            sample_cursor += block;
            assert!(engine.process(&[&in_blk, &in_blk], &mut [&mut out_l, &mut out_r]));
            out_l_all.extend_from_slice(&out_l);
            out_r_all.extend_from_slice(&out_r);
        }
    }

    assert!(out_l_all.iter().all(|s| s.is_finite()));
    assert!(out_r_all.iter().all(|s| s.is_finite()));

    let mut max_step_l = 0.0_f32;
    let mut max_step_idx = 0_usize;
    for (idx, w) in out_l_all.windows(2).enumerate() {
        let d = (w[1] - w[0]).abs();
        if d > max_step_l {
            max_step_l = d;
            max_step_idx = idx;
        }
    }
    let max_step_r = out_r_all
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0_f32, f32::max);

    assert!(
        max_step_l < 0.20 && max_step_r < 0.20,
        "mode switch cycle caused click discontinuity at idx={max_step_idx} (prev={}, curr={}): max_step_l={max_step_l}, max_step_r={max_step_r}",
        out_l_all[max_step_idx],
        out_l_all[max_step_idx + 1]
    );
}

#[test]
fn test_linear_phase_mono_and_stereo_processing_and_solo() {
    use equaform_common::ProcessingMode;

    // 1. Mono impulse & boost check
    let mut mono_engine = Box::new(EngineLifecycle::new());
    mono_engine.prepare(48_000.0, 256, 1);
    let mut params = EqParameters::default();
    params.global.processing_mode = ProcessingMode::LinearPhaseLow;
    params.bands[0].enabled = true;
    params.bands[0].filter_type = FilterType::Bell;
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 6.0;
    params.bands[0].q = 1.0;
    mono_engine.set_parameters(params);

    let mut imp = vec![0.0_f32; 4096];
    imp[0] = 1.0;
    let mut out_m = vec![0.0_f32; 256];
    let mut collected = Vec::with_capacity(4096);
    for chunk in imp.chunks_exact(256) {
        assert!(mono_engine.process(&[chunk], &mut [&mut out_m]));
        collected.extend_from_slice(&out_m);
    }
    let peak_idx = collected
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .map(|(i, _)| i)
        .unwrap_or(0);
    assert_eq!(peak_idx, 1024);
    assert!(
        collected[1024] > 1.01,
        "expected boosted impulse center > 1.01, got {}",
        collected[1024]
    );

    // 2. Stereo Band Solo in LinearPhaseLow isolates target frequency and aligns latency
    let mut stereo_engine = Box::new(EngineLifecycle::new());
    stereo_engine.prepare(48_000.0, 256, 2);
    let mut solo_params = EqParameters::default();
    solo_params.global.processing_mode = ProcessingMode::LinearPhaseLow;
    solo_params.bands[0].frequency_hz = 1000.0;
    solo_params.bands[0].q = 2.0;
    solo_params.set_band_solo(0, true);
    stereo_engine.set_parameters(solo_params);

    let sig_1k = solo_test_sine(1000.0, 48_000.0, 256 * 16);
    let sig_100 = solo_test_sine(100.0, 48_000.0, 256 * 16);
    let out_1k = run_stereo_blocks(&mut stereo_engine, &sig_1k, 256);
    stereo_engine.reset();
    let out_100 = run_stereo_blocks(&mut stereo_engine, &sig_100, 256);

    let in_rms = solo_test_rms(&sig_1k[..256]);
    assert!(solo_test_rms(&out_1k) > 0.7 * in_rms);
    assert!(solo_test_rms(&out_100) < 0.25 * in_rms);
}

#[test]
fn test_linear_phase_automation_reset_and_large_block_dry_delay() {
    use equaform_common::ProcessingMode;
    use equaform_common::automation::AutomationEvent;

    // Verify large block (> 2048 samples) bypass dry delay line works without wraparound
    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(48_000.0, 4096, 2);
    let mut params = EqParameters::default();
    params.global.processing_mode = ProcessingMode::LinearPhaseLow;
    params.global.bypass = true;
    engine.set_parameters(params);

    let mut in_l = vec![0.0_f32; 4096];
    let in_r = vec![0.0_f32; 4096];
    in_l[0] = 1.0;
    let mut out_l = vec![0.0_f32; 4096];
    let mut out_r = vec![0.0_f32; 4096];
    assert!(engine.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
    assert!((out_l[1024] - 1.0).abs() < 1e-6);
    assert!(out_l[0].abs() < 1e-6);

    // Reset and switch to active LinearPhaseLow with automation boost
    engine.reset();
    params.global.bypass = false;
    params.bands[0].enabled = true;
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 0.0;
    params.bands[0].filter_type = FilterType::Bell;
    params.bands[0].q = 1.0;
    engine.set_parameters(params);

    let gain_id = params.bands[0].gain_param_id();
    engine.push_automation_event(AutomationEvent::new(gain_id, 1.0, 0, 1));

    let sine = solo_test_sine(1000.0, 48_000.0, 4096);
    assert!(engine.process(&[&sine, &sine], &mut [&mut out_l, &mut out_r]));
    assert_eq!(engine.eq_parameters().bands[0].gain_db, 24.0);
    // Steady state tail (after latency settle) must reflect the +24 dB automated boost
    assert!(solo_test_rms(&out_l[2048..]) > 5.0 * solo_test_rms(&sine[..2048]));
}

#[test]
fn test_output_gain_linear_cache_accuracy_and_reuse() {
    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(48_000.0, 64, 2);

    let in_l = [0.5_f32; 64];
    let in_r = [0.5_f32; 64];
    let mut out_l = [0.0_f32; 64];
    let mut out_r = [0.0_f32; 64];

    let mut params = EqParameters::default();
    params.global.output_gain_db = 6.0;
    engine.set_parameters(params);

    assert!(engine.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
    let expected_6db = 10.0_f32.powf(6.0 / 20.0);
    assert_eq!(
        engine.coeff_cache.last_output_gain_db.to_bits(),
        6.0_f32.to_bits()
    );
    assert_eq!(
        engine.coeff_cache.output_gain_linear.to_bits(),
        expected_6db.to_bits()
    );
    assert!((out_l[0] - 0.5 * expected_6db).abs() < 1e-6);

    // Inject a sentinel linear value while keeping the cached dB key intact:
    // repeated blocks with the same output_gain_db must reuse the cache without recomputing.
    engine.coeff_cache.output_gain_linear = 0.25;
    engine.coeff_cache.current_output_gain_linear = 0.25;
    assert!(engine.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
    assert_eq!(
        engine.coeff_cache.output_gain_linear.to_bits(),
        0.25_f32.to_bits()
    );
    assert!((out_l[0] - 0.125).abs() < 1e-6);

    // Changing output_gain_db invalidates the cache, recomputes exact 10^(db/20),
    // and smoothly converges to the target linear gain without a step click.
    params.global.output_gain_db = -3.0;
    engine.set_parameters(params);
    for _ in 0..12 {
        assert!(engine.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
    }
    let expected_neg3db = 10.0_f32.powf(-3.0 / 20.0);
    assert_eq!(
        engine.coeff_cache.last_output_gain_db.to_bits(),
        (-3.0_f32).to_bits()
    );
    assert_eq!(
        engine.coeff_cache.output_gain_linear.to_bits(),
        expected_neg3db.to_bits()
    );
    assert!((out_l[63] - 0.5 * expected_neg3db).abs() < 1e-4);
}

#[test]
fn test_output_gain_step_is_smoothed() {
    let sr = 48_000.0;
    let n = 256;
    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(sr, n, 2);
    let mut params = EqParameters::default();
    engine.set_parameters(params);

    let (mut in_l, mut in_r, mut out_l, mut out_r) = (
        vec![0.0f32; n],
        vec![0.0f32; n],
        vec![0.0f32; n],
        vec![0.0f32; n],
    );
    let mut max_step = 0.0f32;
    let mut prev_last_l = 0.0f32;
    let mut idx = 0usize;

    for blk in 0..6 {
        for i in 0..n {
            let ph = 2.0 * std::f64::consts::PI * 1000.0 * (idx as f64 / sr);
            in_l[i] = (0.25 * ph.sin()) as f32;
            in_r[i] = in_l[i];
            idx += 1;
        }
        if blk == 2 {
            params.global.output_gain_db = 12.0;
            engine.set_parameters(params);
        }
        assert!(engine.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
        if blk >= 2 {
            max_step = max_step.max((out_l[0] - prev_last_l).abs());
            for w in out_l.windows(2) {
                max_step = max_step.max((w[1] - w[0]).abs());
            }
        }
        prev_last_l = out_l[n - 1];
    }

    let peak = out_l.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        max_step < 0.14,
        "block-boundary step click: max_step={max_step}"
    );
    assert!(peak > 0.90, "peak after 4 blocks too low: {peak}");
}

#[test]
fn test_band_toggle_and_routing_change_are_click_free() {
    use equaform_common::BandRouting;

    let sr = 48_000.0;
    let n = 256;
    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(sr, n, 2);
    let mut params = EqParameters::default();
    params.bands[0].enabled = true;
    params.bands[0].filter_type = FilterType::Bell;
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 12.0;
    params.bands[0].q = 4.0;
    engine.set_parameters(params);

    let (mut in_l, mut in_r, mut out_l, mut out_r) = (
        vec![0.0f32; n],
        vec![0.0f32; n],
        vec![0.0f32; n],
        vec![0.0f32; n],
    );
    let mut max_step = 0.0f32;
    let mut prev_last_l = 0.0f32;
    let mut idx = 0usize;

    for blk in 0..10 {
        for i in 0..n {
            let ph = 2.0 * std::f64::consts::PI * 1000.0 * (idx as f64 / sr);
            in_l[i] = (0.3 * ph.sin()) as f32;
            in_r[i] = in_l[i];
            idx += 1;
        }
        match blk {
            4 => {
                params.bands[0].enabled = false;
                engine.set_parameters(params);
            }
            6 => {
                params.bands[0].enabled = true;
                engine.set_parameters(params);
            }
            8 => {
                params.set_band_routing(0, BandRouting::Mid);
                engine.set_parameters(params);
            }
            _ => {}
        }
        assert!(engine.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
        if blk >= 1 {
            max_step = max_step.max((out_l[0] - prev_last_l).abs());
            for w in out_l.windows(2) {
                max_step = max_step.max((w[1] - w[0]).abs());
            }
        }
        prev_last_l = out_l[n - 1];
    }

    assert!(
        max_step < 0.20,
        "click detected on band toggle/routing: max_step={max_step}"
    );
}

#[test]
fn test_minimum_to_linear_phase_has_no_silence_dip() {
    use equaform_common::ProcessingMode;

    let sr = 48_000.0;
    let n = 256;
    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(sr, n, 2);
    let mut params = EqParameters::default();
    params.global.processing_mode = ProcessingMode::MinimumPhase;
    params.bands[0].enabled = true;
    params.bands[0].filter_type = FilterType::Bell;
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 3.0;
    params.bands[0].q = 1.0;
    engine.set_parameters(params);

    let (mut in_l, mut in_r, mut out_l, mut out_r) = (
        vec![0.0f32; n],
        vec![0.0f32; n],
        vec![0.0f32; n],
        vec![0.0f32; n],
    );
    let mut idx = 0usize;
    let mut prev_last = 0.0f32;

    for blk in 0..24 {
        for i in 0..n {
            let ph = 2.0 * std::f64::consts::PI * 440.0 * (idx as f64 / sr);
            in_l[i] = (0.5 * ph.sin()) as f32;
            in_r[i] = in_l[i];
            idx += 1;
        }
        if blk == 4 {
            params.global.processing_mode = ProcessingMode::LinearPhaseHigh;
            engine.set_parameters(params);
        }
        assert!(engine.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
        if blk >= 4 {
            let rms = (out_l.iter().map(|s| s * s).sum::<f32>() / n as f32).sqrt();
            assert!(rms > 0.15, "silence dip at block {blk}: rms={rms}");
            let mut max_step = (out_l[0] - prev_last).abs();
            for w in out_l.windows(2) {
                max_step = max_step.max((w[1] - w[0]).abs());
            }
            assert!(max_step < 0.15, "step too large at block {blk}: {max_step}");
        }
        prev_last = out_l[n - 1];
    }
}

#[test]
fn test_linear_phase_auto_gain_waits_for_kernel_ready() {
    use equaform_common::ProcessingMode;

    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(48_000.0, 256, 2);
    let mut params = EqParameters::default();
    params.global.processing_mode = ProcessingMode::LinearPhaseLow;
    params.global.auto_gain = true;
    params.bands[0].enabled = true;
    params.bands[0].filter_type = FilterType::Bell;
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 6.0;
    params.bands[0].q = 1.0;
    engine.set_parameters(params);

    let in_l = vec![0.5f32; 256];
    let in_r = vec![0.5f32; 256];
    let mut out_l = vec![0.0f32; 256];
    let mut out_r = vec![0.0f32; 256];
    for _ in 0..4 {
        assert!(engine.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
    }
    let initial_auto_gain = engine.coeff_cache.auto_gain_db;
    params.bands[0].gain_db = 12.0;
    engine.set_parameters(params);
    assert!(engine.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
    assert!(
        (engine.coeff_cache.auto_gain_db - initial_auto_gain).abs() < 1e-6,
        "auto_gain_db jumped immediately from {initial_auto_gain} to {} before async kernel swap",
        engine.coeff_cache.auto_gain_db
    );
}

#[test]
fn test_lr_routing_switch_is_click_free() {
    use equaform_common::BandRouting;

    let sr = 48_000.0;
    let n = 256;
    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(sr, n, 2);
    let mut params = EqParameters::default();
    params.bands[0].enabled = true;
    params.bands[0].filter_type = FilterType::Bell;
    params.bands[0].frequency_hz = 1137.0;
    params.bands[0].gain_db = 12.0;
    params.bands[0].q = 4.0;
    params.set_band_routing(0, BandRouting::Stereo);
    engine.set_parameters(params);

    let (mut in_l, mut in_r, mut out_l, mut out_r) = (
        vec![0.0f32; n],
        vec![0.0f32; n],
        vec![0.0f32; n],
        vec![0.0f32; n],
    );
    let mut max_step_l = 0.0f32;
    let mut max_step_r = 0.0f32;
    let mut prev_last_l = 0.0f32;
    let mut prev_last_r = 0.0f32;
    let mut idx = 0usize;

    for blk in 0..12 {
        for i in 0..n {
            let ph = 2.0 * std::f64::consts::PI * 1137.0 * (idx as f64 / sr);
            in_l[i] = (0.3 * ph.sin()) as f32;
            in_r[i] = (0.3 * ph.sin()) as f32;
            idx += 1;
        }
        match blk {
            3 => {
                params.set_band_routing(0, BandRouting::Left);
                engine.set_parameters(params);
            }
            6 => {
                params.set_band_routing(0, BandRouting::Right);
                engine.set_parameters(params);
            }
            9 => {
                params.set_band_routing(0, BandRouting::Stereo);
                engine.set_parameters(params);
            }
            _ => {}
        }
        assert!(engine.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
        if blk >= 1 {
            max_step_l = max_step_l.max((out_l[0] - prev_last_l).abs());
            max_step_r = max_step_r.max((out_r[0] - prev_last_r).abs());
            for w in out_l.windows(2) {
                max_step_l = max_step_l.max((w[1] - w[0]).abs());
            }
            for w in out_r.windows(2) {
                max_step_r = max_step_r.max((w[1] - w[0]).abs());
            }
        }
        prev_last_l = out_l[n - 1];
        prev_last_r = out_r[n - 1];
    }

    assert!(
        max_step_l < 0.22 && max_step_r < 0.22,
        "L/R routing switch click: max_step_l={max_step_l}, max_step_r={max_step_r}"
    );
}

#[test]
fn test_solo_lr_routing_switch_is_click_free() {
    use equaform_common::BandRouting;

    let sr = 48_000.0;
    let n = 256;
    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(sr, n, 2);
    let mut params = EqParameters::default();
    params.bands[0].enabled = true;
    params.bands[0].filter_type = FilterType::Bell;
    params.bands[0].frequency_hz = 1137.0;
    params.bands[0].gain_db = 12.0;
    params.bands[0].q = 4.0;
    params.set_band_solo(0, true);
    params.set_band_routing(0, BandRouting::Stereo);
    engine.set_parameters(params);

    let (mut in_l, mut in_r, mut out_l, mut out_r) = (
        vec![0.0f32; n],
        vec![0.0f32; n],
        vec![0.0f32; n],
        vec![0.0f32; n],
    );
    let mut max_step_l = 0.0f32;
    let mut max_step_r = 0.0f32;
    let mut prev_last_l = 0.0f32;
    let mut prev_last_r = 0.0f32;
    let mut idx = 0usize;

    for blk in 0..18 {
        for i in 0..n {
            let ph = 2.0 * std::f64::consts::PI * 1137.0 * (idx as f64 / sr);
            in_l[i] = (0.3 * ph.sin()) as f32;
            in_r[i] = (0.2 * (ph * 1.3).sin()) as f32;
            idx += 1;
        }
        match blk {
            3 => {
                params.set_band_routing(0, BandRouting::Left);
                engine.set_parameters(params);
            }
            6 => {
                params.set_band_routing(0, BandRouting::Right);
                engine.set_parameters(params);
            }
            9 => {
                params.set_band_routing(0, BandRouting::Mid);
                engine.set_parameters(params);
            }
            12 => {
                params.set_band_routing(0, BandRouting::Side);
                engine.set_parameters(params);
            }
            15 => {
                params.set_band_routing(0, BandRouting::Stereo);
                engine.set_parameters(params);
            }
            _ => {}
        }
        assert!(engine.process(&[&in_l, &in_r], &mut [&mut out_l, &mut out_r]));
        if blk >= 1 {
            max_step_l = max_step_l.max((out_l[0] - prev_last_l).abs());
            max_step_r = max_step_r.max((out_r[0] - prev_last_r).abs());
            for w in out_l.windows(2) {
                max_step_l = max_step_l.max((w[1] - w[0]).abs());
            }
            for w in out_r.windows(2) {
                max_step_r = max_step_r.max((w[1] - w[0]).abs());
            }
        }
        prev_last_l = out_l[n - 1];
        prev_last_r = out_r[n - 1];
    }

    assert!(
        max_step_l < 0.15 && max_step_r < 0.15,
        "Solo L/R routing switch click: max_step_l={max_step_l}, max_step_r={max_step_r}"
    );
}

#[test]
fn test_linear_phase_unsolo_has_no_stale_audio_burst() {
    use equaform_common::ProcessingMode;

    let sr = 48_000.0;
    let n = 256;
    let mut engine = Box::new(EngineLifecycle::new());
    engine.prepare(sr, n, 2);
    let mut params = EqParameters::default();
    params.global.processing_mode = ProcessingMode::LinearPhaseLow;
    params.bands[0].enabled = true;
    params.bands[0].filter_type = FilterType::Bell;
    params.bands[0].frequency_hz = 1000.0;
    params.bands[0].gain_db = 6.0;
    params.bands[0].q = 1.0;
    engine.set_parameters(params);

    let loud = vec![0.5f32; n];
    let silent = vec![0.0f32; n];
    let mut out_l = vec![0.0f32; n];
    let mut out_r = vec![0.0f32; n];

    // Fill LinearPhaseLow pipeline with loud audio.
    for _ in 0..8 {
        assert!(engine.process(&[&loud, &loud], &mut [&mut out_l, &mut out_r]));
    }

    // Engage Solo and feed silence for 12 blocks (longer than 1024-sample latency).
    params.set_band_solo(0, true);
    engine.set_parameters(params);
    for _ in 0..12 {
        assert!(engine.process(&[&silent, &silent], &mut [&mut out_l, &mut out_r]));
    }

    // Disengage Solo while still feeding silence: no stale loud audio should leak out.
    params.set_band_solo(0, false);
    engine.set_parameters(params);
    let mut max_leak = 0.0f32;
    for _ in 0..6 {
        assert!(engine.process(&[&silent, &silent], &mut [&mut out_l, &mut out_r]));
        for &s in &out_l {
            max_leak = max_leak.max(s.abs());
        }
    }

    assert!(
        max_leak < 1e-3,
        "stale LinearPhase audio burst leaked on un-solo: max_leak={max_leak}"
    );
}
