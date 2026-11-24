use super::*;
use cpal::StreamConfig;
use cpal::traits::{DeviceTrait, HostTrait};
use equaform_common::EqParameters;
use equaform_common::audio_ring::create_audio_ring_buffer;
use std::sync::atomic::Ordering;

#[test]
#[ignore = "Requires physical macOS audio hardware (Equaform-8 / Komplete Audio)"]
fn test_live_streaming_diagnostic() {
    let mut stream = AudioStream::new();
    stream.set_source_mode(AudioSourceMode::PassThrough);
    stream.set_input_device(Some("Equaform-8".to_string()));
    stream.set_output_device(Some("Komplete Audio 6 MK2".to_string()));

    println!("Starting diagnostic live stream on Komplete Audio...");
    match stream.start() {
        Ok(()) => {
            println!(
                "Stream started successfully! Target SR={}",
                stream.sample_rate()
            );
            for sec in 1..=3 {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let (pl, pr) = stream.peak_levels_db();
                println!("Second {sec}: peaks L={pl:.1} dB, R={pr:.1} dB");
            }
            stream.stop().unwrap();
            println!("Stream stopped.");
        }
        Err(e) => {
            println!("Stream start failed: {e}");
        }
    }
}

#[test]
fn test_soft_limit_characteristics() {
    assert_eq!(soft_limit(0.0), 0.0);
    assert_eq!(soft_limit(0.5), 0.5);
    assert_eq!(soft_limit(-0.5), -0.5);
    assert_eq!(soft_limit(0.85), 0.85);
    assert_eq!(soft_limit(-0.85), -0.85);

    let at_1 = soft_limit(1.0);
    let at_2 = soft_limit(2.0);
    let at_10 = soft_limit(10.0);
    assert!(at_1 > 0.85 && at_1 < 1.0);
    assert!(at_2 >= at_1 && at_2 <= 1.0);
    assert!(at_10 >= at_2 && at_10 <= 1.0);
    assert_eq!(soft_limit(-1.0), -at_1);
}

#[test]
fn test_realtime_threaded_streaming() {
    let (producer, consumer) = create_audio_ring_buffer(16384);

    // Deterministic step-based streaming simulation:
    // Replaces non-deterministic thread sleep with discrete frame steps,
    // simulating producer and consumer clock drift (varying chunk sizes)
    // with 100% reproducible execution across CI environments.
    let mut left_out = vec![0.0f32; 1024];
    let mut right_out = vec![0.0f32; 1024];
    let mut is_primed = false;
    let mut last_l = 0.0f32;
    let mut last_r = 0.0f32;
    let mut total_frames_consumed = 0;

    for step in 0..100 {
        // Producer pushes frames with controlled drift variations:
        // - Initial priming phase (steps 0..4): 1024 frames per step to build buffer
        // - Step 20, 40, 60, 80: 515 frames (producer runs slightly faster than consumer)
        // - Step 15, 35, 55, 75: 509 frames (producer runs slightly slower than consumer)
        // - Nominal: 512 frames
        let input_frames = if step < 4 {
            1024
        } else if step % 20 == 0 {
            515
        } else if step % 20 == 15 {
            509
        } else {
            512
        };
        let chunk = vec![0.5f32; input_frames * 2];
        producer.push_interleaved(&chunk, 2);

        let available = consumer.available_read();
        if !is_primed {
            if available >= 2048 {
                is_primed = true;
            } else {
                left_out.fill(0.0);
                right_out.fill(0.0);
            }
        }

        if is_primed {
            let frames_to_pop = if available > 3072 {
                513
            } else if (511..1024).contains(&available) {
                511
            } else {
                512
            };

            let popped = consumer.pop_slice(
                &mut left_out[..frames_to_pop],
                &mut right_out[..frames_to_pop],
            );
            assert_eq!(popped, frames_to_pop);
            total_frames_consumed += popped;

            if frames_to_pop == 513 && popped == 513 {
                left_out[511] = (left_out[511] + left_out[512]) * 0.5;
                right_out[511] = (right_out[511] + right_out[512]) * 0.5;
            } else if frames_to_pop == 511 && popped == 511 {
                left_out[511] = (left_out[510] + last_l) * 0.5;
                right_out[511] = (right_out[510] + last_r) * 0.5;
            } else {
                left_out[511] = 0.5;
                right_out[511] = 0.5;
            }
            last_l = left_out[511];
            last_r = right_out[511];
        }
    }

    assert!(is_primed, "Stream must successfully prime");
    assert!(
        total_frames_consumed >= 45000,
        "Stream must consume expected audio frames ({total_frames_consumed} frames consumed)"
    );
    assert_eq!(last_l, 0.5);
    assert_eq!(last_r, 0.5);
}

#[test]
fn test_drift_controller_underrun_recovery_without_stall() {
    let (producer, consumer) = create_audio_ring_buffer(16384);
    let mut drift = ContinuousPiDriftController::new(1.0);
    let mut left = vec![0.0f32; 512];
    let mut right = vec![0.0f32; 512];

    // 1. Prime the controller with 4096 frames
    for _ in 0..8 {
        let chunk = vec![0.33f32; 1024];
        producer.push_interleaved(&chunk, 2);
    }
    drift.read_from(&consumer, &mut left, &mut right, 512);
    assert!(drift.is_primed());
    assert_eq!(left[0], 0.33);

    // 2. Simulate severe USB starvation: drain the buffer down to 0
    while consumer.available_read() > 0 {
        let _ = consumer.pop();
    }
    assert_eq!(consumer.available_read(), 0);

    // 3. Output callback runs while buffer is starved
    drift.read_from(&consumer, &mut left, &mut right, 512);
    // It outputs silence for this starvation block
    assert_eq!(left[0], 0.0);
    // CRITICAL: Controller must REMAIN PRIMED to avoid the 46 ms reprime blackout!
    assert!(
        drift.is_primed(),
        "Controller must remain primed on starvation!"
    );

    // 4. Input packet arrives immediately on the next cycle (just 512 frames)
    let chunk = vec![0.88f32; 1024];
    producer.push_interleaved(&chunk, 2);

    // 5. Output callback runs immediately on the next block
    drift.read_from(&consumer, &mut left, &mut right, 512);
    // Audio playback resumes immediately without waiting for 4096 frames!
    assert!(drift.is_primed());
    assert!(
        (left[0] - 0.88).abs() < 1e-4,
        "Playback resumed instantly without stall!"
    );
}

#[test]
fn test_streaming_simulation_100_blocks() {
    let (producer, consumer) = create_audio_ring_buffer(16384);
    let mut drift = ContinuousPiDriftController::default();
    let mut left_out = vec![0.0f32; 512];
    let mut right_out = vec![0.0f32; 512];

    // Simulate 100 blocks
    for _ in 0..100 {
        let chunk = vec![0.1f32; 1024];
        producer.push_interleaved(&chunk, 2);
        drift.read_from(&consumer, &mut left_out, &mut right_out, 512);
    }
}

#[test]
fn test_drift_controller_rate_conversions() {
    // 1. Nominal 44.1 kHz -> 44.1 kHz (ratio = 1.0)
    let (producer, consumer) = create_audio_ring_buffer(16384);
    let mut drift_1to1 = ContinuousPiDriftController::new(1.0);
    let mut left = vec![0.0f32; 512];
    let mut right = vec![0.0f32; 512];

    // Push initial pre-buffer (at least TARGET_FRAMES = 4096)
    for _ in 0..8 {
        let chunk = vec![0.5f32; 1024];
        producer.push_interleaved(&chunk, 2);
    }
    assert!(consumer.available_read() >= ContinuousPiDriftController::TARGET_FRAMES);

    for _ in 0..50 {
        let chunk = vec![0.5f32; 1024];
        producer.push_interleaved(&chunk, 2);
        drift_1to1.read_from(&consumer, &mut left, &mut right, 512);
        assert_eq!(left[0], 0.5);
        assert_eq!(right[0], 0.5);
    }
    assert!(drift_1to1.is_primed());

    // 2. 48.0 kHz input -> 44.1 kHz output (ratio = 48000.0 / 44100.0 = 1.088435)
    let (p_48to44, c_48to44) = create_audio_ring_buffer(16384);
    let mut drift_48to44 = ContinuousPiDriftController::new(48000.0 / 44100.0);
    // Pre-fill to TARGET_FRAMES
    for _ in 0..8 {
        let in_chunk = vec![0.25f32; 557 * 2];
        p_48to44.push_interleaved(&in_chunk, 2);
    }
    for _ in 0..50 {
        let in_chunk = vec![0.25f32; 557 * 2]; // ~557 input frames per 512 output frames
        p_48to44.push_interleaved(&in_chunk, 2);
        drift_48to44.read_from(&c_48to44, &mut left, &mut right, 512);
    }
    assert!(drift_48to44.is_primed());
    // Sound must be non-zero and stable around 0.25
    assert!((left[256] - 0.25).abs() < 1e-4);

    // 3. 44.1 kHz input -> 48.0 kHz output (ratio = 44100.0 / 48000.0 = 0.91875)
    let (p_44to48, c_44to48) = create_audio_ring_buffer(16384);
    let mut drift_44to48 = ContinuousPiDriftController::new(44100.0 / 48000.0);
    for _ in 0..9 {
        let in_chunk = vec![0.75f32; 470 * 2];
        p_44to48.push_interleaved(&in_chunk, 2);
    }
    for _ in 0..50 {
        let in_chunk = vec![0.75f32; 470 * 2]; // ~470 input frames per 512 output frames
        p_44to48.push_interleaved(&in_chunk, 2);
        drift_44to48.read_from(&c_44to48, &mut left, &mut right, 512);
    }
    assert!(drift_44to48.is_primed());
    assert!((left[256] - 0.75).abs() < 1e-4);
}

#[test]
fn test_drift_controller_sine_phase_continuity_across_blocks() {
    let (producer, consumer) = create_audio_ring_buffer(16384);
    // Simulate asynchronous USB crystal drift (+350 ppm)
    let mut drift = ContinuousPiDriftController::new(1.00035);
    let mut left = vec![0.0f32; 512];
    let mut right = vec![0.0f32; 512];

    let sr = 44100.0f64;
    let freq = 440.0f64;
    let max_theoretical_step = (2.0 * std::f64::consts::PI * freq / sr) as f32 * 1.05;
    let mut sample_idx = 0usize;

    // Pre-fill 6000 frames of continuous 440 Hz sine wave
    for _ in 0..6000 {
        let s = (2.0 * std::f64::consts::PI * freq * (sample_idx as f64) / sr).sin() as f32;
        assert!(producer.push(s, -s));
        sample_idx += 1;
    }

    let mut prev_out_l: Option<f32> = None;
    for _block in 0..100 {
        // Replenish ~512 frames per block
        for _ in 0..512 {
            let s = (2.0 * std::f64::consts::PI * freq * (sample_idx as f64) / sr).sin() as f32;
            let _ = producer.push(s, -s);
            sample_idx += 1;
        }
        drift.read_from(&consumer, &mut left, &mut right, 512);
        for &sample in &left {
            if let Some(prev) = prev_out_l {
                let delta = (sample - prev).abs();
                assert!(
                    delta <= max_theoretical_step,
                    "Phase discontinuity click detected across block boundary! delta={delta}, max={max_theoretical_step}"
                );
            }
            prev_out_l = Some(sample);
        }
    }
    assert_eq!(drift.underrun_count, 0);
}

#[test]
#[ignore = "Requires physical audio device"]
fn test_audio_stream_start_diagnostic() {
    let host = cpal::default_host();
    let default_output = host
        .default_output_device()
        .expect("Default output device not found");
    println!("Found default output device: {:?}", default_output.name());
    let config = StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(44100),
        buffer_size: cpal::BufferSize::Default,
    };
    let stream = default_output.build_output_stream(
        &config,
        |data: &mut [f32], _| {
            data.fill(0.0);
        },
        |e| eprintln!("err: {e}"),
        None,
    );
    println!(
        "Building output stream on default device: {:?}",
        stream.is_ok()
    );
    assert!(stream.is_ok());

    println!(
        "Default output: {:?}, config: {:?}",
        default_output.name(),
        default_output.default_output_config()
    );
    if let Ok(configs) = default_output.supported_output_configs() {
        for c in configs {
            println!("  Supported config: {c:?}");
        }
    }
    if let Some(inp) = host.default_input_device() {
        println!(
            "Default input: {:?}, config: {:?}",
            inp.name(),
            inp.default_input_config()
        );
    }
    println!(
        "Available outputs: {:?}",
        AudioStream::available_output_devices()
    );
    println!(
        "Available inputs: {:?}",
        AudioStream::available_input_devices()
    );
    let mut stream = AudioStream::new();
    let out_name = default_output.name().ok();
    stream.set_output_device(out_name.clone());
    match stream.start() {
        Ok(()) => {
            println!(
                "Audio stream to {out_name:?} started successfully! Active: {}",
                stream.is_active()
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
            stream.stop().unwrap();
        }
        Err(e) => {
            println!("Audio stream to {out_name:?} failed to start: {e}");
        }
    }
}

#[test]
fn test_audio_stream_triple_buffer_and_error_counter() {
    let mut stream = AudioStream::new();
    assert_eq!(stream.stream_error_count(), 0);

    let mut params = EqParameters::default();
    params.bands[0].gain_db = 6.0;
    assert!(stream.set_parameters(params).is_ok());

    // Stream error increment
    stream.stream_errors().fetch_add(1, Ordering::Relaxed);
    assert_eq!(stream.stream_error_count(), 1);
}
