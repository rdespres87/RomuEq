//! Real-time-safe linear-phase FIR equalizer module.
//!
//! Uses a Uniformly Partitioned Overlap-Save (UPOLS) FFT convolution engine
//! with dual-bank crossfading and arbitrary host block-size FIFO buffering.

pub mod convolver;
pub mod synthesis;
mod worker;

pub use convolver::LinearPhaseConvolver;
pub use synthesis::{
    KernelSynthesizer, LINEAR_HIGH_DELAY, LINEAR_HIGH_FFT_SIZE, LINEAR_LOW_DELAY,
    LINEAR_LOW_FFT_SIZE, PartitionBank, UPOLS_BLOCK_SIZE, UPOLS_FFT_SIZE, UPOLS_MAX_PARTITIONS,
    biquad_magnitude_linear, blackman_harris,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BiquadCoefficients, FilterSpecification, biquad_response_at_frequency};
    use equaform_common::{
        BandParameters, BandRouting, EqParameters, FilterType, ProcessingMode, Slope,
    };
    use std::f32::consts::PI;

    fn rms(samples: &[f32]) -> f32 {
        let sum_sq: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
        (sum_sq / samples.len() as f64).sqrt() as f32
    }

    fn bell_params(gain_db: f32, freq_hz: f32, q: f32, routing: BandRouting) -> EqParameters {
        let mut params = EqParameters::default();
        params.bands[0] = BandParameters {
            enabled: true,
            filter_type: FilterType::Bell,
            frequency_hz: freq_hz,
            gain_db,
            q,
            slope: Slope::Db12,
        };
        params.set_band_routing(0, routing);
        params
    }

    fn run_impulse_stereo(
        convolver: &mut LinearPhaseConvolver,
        total_len: usize,
        block_size: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let mut left = vec![0.0_f32; total_len];
        let mut right = vec![0.0_f32; total_len];
        left[0] = 1.0;
        right[0] = 1.0;

        for (l_chunk, r_chunk) in left
            .chunks_mut(block_size)
            .zip(right.chunks_mut(block_size))
        {
            convolver.process_stereo(l_chunk, r_chunk);
        }
        (left, right)
    }

    #[test]
    fn test_biquad_magnitude_linear_dc_nyquist_and_nan() {
        let unity = BiquadCoefficients::default();
        assert!((biquad_magnitude_linear(&unity, 0.0) - 1.0).abs() < 1e-6);
        assert!((biquad_magnitude_linear(&unity, PI) - 1.0).abs() < 1e-6);

        let hp = FilterSpecification {
            filter_type: FilterType::HighPass,
            frequency_hz: 1000.0,
            gain_db: 0.0,
            q: 0.707,
            slope: Slope::Db12,
            sample_rate: 48_000.0,
        }
        .compute();
        assert!(biquad_magnitude_linear(&hp, 0.0) < 1e-5);
        assert!((biquad_magnitude_linear(&hp, PI) - 1.0).abs() < 0.05);

        let lp = FilterSpecification {
            filter_type: FilterType::LowPass,
            frequency_hz: 1000.0,
            gain_db: 0.0,
            q: 0.707,
            slope: Slope::Db12,
            sample_rate: 48_000.0,
        }
        .compute();
        assert!((biquad_magnitude_linear(&lp, 0.0) - 1.0).abs() < 1e-5);
        assert!(biquad_magnitude_linear(&lp, PI) < 1e-5);

        let invalid = BiquadCoefficients {
            b0: f32::NAN,
            ..Default::default()
        };
        assert_eq!(biquad_magnitude_linear(&invalid, 0.5), 1.0);
        assert_eq!(biquad_magnitude_linear(&unity, f32::NAN), 1.0);
    }

    #[test]
    fn test_blackman_harris_center_symmetry_and_bounds() {
        let len_low = 2 * LINEAR_LOW_DELAY + 1;
        assert!((blackman_harris(LINEAR_LOW_DELAY, len_low) - 1.0).abs() < 1e-6);
        for n in 0..len_low {
            let diff =
                (blackman_harris(n, len_low) - blackman_harris(len_low - 1 - n, len_low)).abs();
            assert!(diff < 1e-6);
        }
        assert_eq!(blackman_harris(0, 1), 1.0);
        assert_eq!(blackman_harris(len_low, len_low), 0.0);
    }

    #[test]
    fn test_unity_impulse_exact_latency_low_across_block_sizes() {
        let block_sizes = [32, 64, 128, 256, 333, 512];
        for &block_size in &block_sizes {
            let mut convolver = LinearPhaseConvolver::new(512);
            assert_eq!(convolver.current_mode(), ProcessingMode::LinearPhaseLow);
            assert_eq!(convolver.latency_samples(), 1024);

            let (left, right) = run_impulse_stereo(&mut convolver, 4096, block_size);
            for (idx, (&l, &r)) in left.iter().zip(right.iter()).enumerate() {
                if idx == 1024 {
                    assert!(
                        (l - 1.0).abs() < 1e-5 && (r - 1.0).abs() < 1e-5,
                        "block_size={block_size}: expected ~1.0 at 1024, got l={l}, r={r}"
                    );
                } else {
                    assert!(
                        l.abs() < 1e-5 && r.abs() < 1e-5,
                        "block_size={block_size}: expected ~0.0 at {idx}, got l={l}, r={r}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_unity_impulse_exact_latency_high() {
        let block_sizes = [64, 256, 333];
        for &block_size in &block_sizes {
            let mut convolver = LinearPhaseConvolver::new(512);
            convolver.update_kernel(
                &EqParameters::default(),
                48_000.0,
                ProcessingMode::LinearPhaseHigh,
            );
            assert_eq!(convolver.current_mode(), ProcessingMode::LinearPhaseHigh);
            assert_eq!(convolver.latency_samples(), 2048);

            let (left, right) = run_impulse_stereo(&mut convolver, 6144, block_size);
            for (idx, (&l, &r)) in left.iter().zip(right.iter()).enumerate() {
                if idx == 2048 {
                    assert!(
                        (l - 1.0).abs() < 1e-5 && (r - 1.0).abs() < 1e-5,
                        "block_size={block_size}: expected ~1.0 at 2048, got l={l}, r={r}"
                    );
                } else {
                    assert!(
                        l.abs() < 1e-5 && r.abs() < 1e-5,
                        "block_size={block_size}: expected ~0.0 at {idx}, got l={l}, r={r}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_impulse_symmetry_proves_exact_linear_phase() {
        let params = bell_params(6.0, 1000.0, 1.0, BandRouting::Stereo);
        let mut convolver = LinearPhaseConvolver::new(256);
        convolver.update_kernel(&params, 48_000.0, ProcessingMode::LinearPhaseLow);
        convolver.reset();

        let (left, right) = run_impulse_stereo(&mut convolver, 4096, 256);
        assert!(left[1024] > 1.0);
        for k in 1..=LINEAR_LOW_DELAY {
            let diff_l = (left[1024 - k] - left[1024 + k]).abs();
            let diff_r = (right[1024 - k] - right[1024 + k]).abs();
            assert!(
                diff_l < 1e-4 && diff_r < 1e-4,
                "asymmetry at offset k={k}: diff_l={diff_l}, diff_r={diff_r}"
            );
        }
    }

    #[test]
    fn test_steady_state_sine_magnitude_accuracy() {
        let sample_rate = 48_000.0_f32;
        let params = bell_params(6.0, 1000.0, 1.0, BandRouting::Stereo);
        let coeffs = FilterSpecification::from_band(&params.bands[0], sample_rate).compute();

        for mode in [
            ProcessingMode::LinearPhaseLow,
            ProcessingMode::LinearPhaseHigh,
        ] {
            let mut convolver = LinearPhaseConvolver::new(256);
            convolver.update_kernel(&params, sample_rate, mode);
            convolver.reset();

            let settle = convolver.latency_samples() as usize + 2048;
            let total_len = settle + 4096;

            for &freq_hz in &[100.0_f32, 1000.0_f32] {
                convolver.reset();
                let input: Vec<f32> = (0..total_len)
                    .map(|i| 0.25 * (2.0 * PI * freq_hz * i as f32 / sample_rate).sin())
                    .collect();
                let mut left = input.clone();
                let mut right = input.clone();

                for (l_chunk, r_chunk) in left.chunks_mut(256).zip(right.chunks_mut(256)) {
                    convolver.process_stereo(l_chunk, r_chunk);
                }

                let latency = convolver.latency_samples() as usize;
                let in_rms = rms(&input[settle - latency..total_len - latency]);
                let out_rms = rms(&left[settle..total_len]);
                let measured_db = 20.0 * (out_rms / in_rms).log10();
                let ref_db = biquad_response_at_frequency(&coeffs, freq_hz, sample_rate);

                assert!(
                    (measured_db - ref_db).abs() < 0.3,
                    "mode={mode:?}, freq={freq_hz}: measured={measured_db:.3} dB, ref={ref_db:.3} dB"
                );
            }
        }
    }

    #[test]
    fn test_mid_side_routing_side_only_boost() {
        let sample_rate = 48_000.0_f32;
        let params = bell_params(6.0, 1000.0, 1.0, BandRouting::Side);
        let coeffs = FilterSpecification::from_band(&params.bands[0], sample_rate).compute();
        let ref_db = biquad_response_at_frequency(&coeffs, 1000.0, sample_rate);

        let mut convolver = LinearPhaseConvolver::new(256);
        convolver.update_kernel(&params, sample_rate, ProcessingMode::LinearPhaseLow);
        convolver.reset();

        let total_len = 8192;
        let settle = 4096;
        let sine: Vec<f32> = (0..total_len)
            .map(|i| 0.25 * (2.0 * PI * 1000.0 * i as f32 / sample_rate).sin())
            .collect();

        // Mono input (L == R): Side component is zero, so output remains ~0 dB.
        let mut mono_l = sine.clone();
        let mut mono_r = sine.clone();
        for (l_c, r_c) in mono_l.chunks_mut(256).zip(mono_r.chunks_mut(256)) {
            convolver.process_stereo(l_c, r_c);
        }
        let in_rms = rms(&sine[settle..total_len]);
        let mono_db = 20.0 * (rms(&mono_l[settle..total_len]) / in_rms).log10();
        assert!(
            mono_db.abs() < 0.1,
            "mono should be unaffected: {mono_db} dB"
        );

        // Anti-phase input (L == -R): purely Side, boosted by ~+6 dB.
        convolver.reset();
        let mut side_l = sine.clone();
        let mut side_r: Vec<f32> = sine.iter().map(|&x| -x).collect();
        for (l_c, r_c) in side_l.chunks_mut(256).zip(side_r.chunks_mut(256)) {
            convolver.process_stereo(l_c, r_c);
        }
        let side_db = 20.0 * (rms(&side_l[settle..total_len]) / in_rms).log10();
        assert!(
            (side_db - ref_db).abs() < 0.3,
            "anti-phase expected ~{ref_db:.3} dB, got {side_db:.3} dB"
        );
        for (&l, &r) in side_l[settle..].iter().zip(side_r[settle..].iter()) {
            assert!((l + r).abs() < 1e-5);
        }
    }

    #[test]
    fn test_dual_bank_kernel_crossfade_smooth() {
        let sample_rate = 48_000.0_f32;
        let mut convolver = LinearPhaseConvolver::new(128);
        let initial_params = bell_params(0.0, 1000.0, 1.0, BandRouting::Stereo);
        convolver.update_kernel(&initial_params, sample_rate, ProcessingMode::LinearPhaseLow);
        convolver.reset();

        let total_blocks = 64;
        let block_size = 128;
        let mut out_l = Vec::with_capacity(total_blocks * block_size);
        let mut out_r = Vec::with_capacity(total_blocks * block_size);

        for blk in 0..total_blocks {
            if blk > 8 && blk <= 24 {
                let gain_db = (blk - 8) as f32 * 0.75;
                let params = bell_params(gain_db, 1000.0, 1.0, BandRouting::Stereo);
                convolver.update_kernel(&params, sample_rate, ProcessingMode::LinearPhaseLow);
            }
            let mut l: Vec<f32> = (0..block_size)
                .map(|i| {
                    let s = blk * block_size + i;
                    0.25 * (2.0 * PI * 1000.0 * s as f32 / sample_rate).sin()
                })
                .collect();
            let mut r = l.clone();
            convolver.process_stereo(&mut l, &mut r);
            out_l.extend_from_slice(&l);
            out_r.extend_from_slice(&r);
        }

        assert!(out_l.iter().all(|s| s.is_finite()));
        assert!(out_r.iter().all(|s| s.is_finite()));

        // A 1 kHz sine at 48 kHz with amplitude <= 1.0 has max sample step ~2*PI*1000/48000*1.0 ~ 0.131.
        // Smooth crossfade must not introduce large step discontinuities.
        let max_delta = out_l
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_delta < 0.25,
            "unexpected discontinuity during kernel crossfade: max_delta={max_delta}"
        );

        let tail_start = out_l.len() - 1024;
        let in_rms = 0.25 * std::f32::consts::FRAC_1_SQRT_2;
        let out_rms = rms(&out_l[tail_start..]);
        let tail_db = 20.0 * (out_rms / in_rms).log10();
        assert!(
            tail_db > 10.0,
            "expected steady-state sweep target ~+12 dB, got {tail_db:.2} dB"
        );
    }

    #[test]
    fn test_process_mono_and_reset() {
        let sample_rate = 48_000.0_f32;
        let params = bell_params(6.0, 1000.0, 1.0, BandRouting::Stereo);
        let mut convolver = LinearPhaseConvolver::new(256);
        convolver.update_kernel(&params, sample_rate, ProcessingMode::LinearPhaseLow);
        convolver.reset();

        let mut impulse = vec![0.0_f32; 4096];
        impulse[0] = 1.0;
        for chunk in impulse.chunks_mut(128) {
            convolver.process_mono(chunk);
        }
        let peak_idx = impulse
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .map(|(i, _)| i)
            .unwrap_or(0);
        assert_eq!(peak_idx, 1024);

        convolver.reset();
        let mut silence = vec![0.0_f32; 2048];
        for chunk in silence.chunks_mut(256) {
            convolver.process_mono(chunk);
        }
        assert!(silence.iter().all(|&s| s.abs() < 1e-7));
    }

    #[test]
    fn test_oversized_host_block_and_minimum_phase_passthrough() {
        // Test processing a block larger than max_host_block_size without panics or allocations.
        let mut convolver = LinearPhaseConvolver::new(128);
        let mut left = vec![0.0_f32; 4096];
        let mut right = vec![0.0_f32; 4096];
        left[0] = 1.0;
        right[0] = 1.0;
        convolver.process_stereo(&mut left, &mut right);
        assert!((left[1024] - 1.0).abs() < 1e-5);
        assert!((right[1024] - 1.0).abs() < 1e-5);

        // Switching to MinimumPhase sets 0 latency and passes through unchanged.
        convolver.update_kernel(
            &EqParameters::default(),
            48_000.0,
            ProcessingMode::MinimumPhase,
        );
        assert_eq!(convolver.current_mode(), ProcessingMode::MinimumPhase);
        assert_eq!(convolver.latency_samples(), 0);
        let mut mono = vec![0.25_f32, -0.5, 0.75];
        let expected = mono.clone();
        convolver.process_mono(&mut mono);
        assert_eq!(mono, expected);
    }

    #[test]
    fn test_async_kernel_update_applies_eq_gain_click_free() {
        use std::time::{Duration, Instant};

        let sample_rate = 48_000.0_f32;
        let block_size = 256_usize;
        let mut convolver = LinearPhaseConvolver::new(block_size);
        let mut out_l = Vec::new();
        let mut out_r = Vec::new();
        let mut sample_idx = 0_usize;

        let mut push_sine_block = |conv: &mut LinearPhaseConvolver, cursor: &mut usize| {
            let mut l: Vec<f32> = (0..block_size)
                .map(|i| 0.25 * (2.0 * PI * 1000.0 * (*cursor + i) as f32 / sample_rate).sin())
                .collect();
            let mut r = l.clone();
            *cursor += block_size;
            conv.process_stereo(&mut l, &mut r);
            out_l.extend_from_slice(&l);
            out_r.extend_from_slice(&r);
        };

        for _ in 0..8 {
            push_sine_block(&mut convolver, &mut sample_idx);
        }

        let target_params = bell_params(6.0, 1000.0, 1.0, BandRouting::Stereo);
        convolver.request_kernel_update(
            &target_params,
            sample_rate,
            ProcessingMode::LinearPhaseLow,
        );
        let target_gen = convolver.worker_next_generation();

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut settled_blocks = 0_usize;
        while settled_blocks < 10 {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for async kernel update"
            );
            push_sine_block(&mut convolver, &mut sample_idx);
            if convolver.worker_last_polled_generation() >= target_gen
                && !convolver.is_crossfade_pending()
            {
                settled_blocks += 1;
            } else {
                std::thread::yield_now();
            }
        }

        assert!(out_l.iter().all(|s| s.is_finite()));
        assert!(out_r.iter().all(|s| s.is_finite()));

        let max_delta = out_l
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_delta < 0.25,
            "async kernel update caused discontinuity: max_delta={max_delta}"
        );

        let coeffs = FilterSpecification::from_band(&target_params.bands[0], sample_rate).compute();
        let ref_db = biquad_response_at_frequency(&coeffs, 1000.0, sample_rate);
        let in_rms = 0.25 * std::f32::consts::FRAC_1_SQRT_2;
        let out_rms = rms(&out_l[out_l.len() - 1024..]);
        let measured_db = 20.0 * (out_rms / in_rms).log10();
        assert!(
            (measured_db - ref_db).abs() < 0.3,
            "expected ~{ref_db:.3} dB, got {measured_db:.3} dB"
        );
    }

    #[test]
    fn test_async_kernel_coalesces_rapid_automation_requests() {
        use std::time::{Duration, Instant};

        let sample_rate = 48_000.0_f32;
        let block_size = 256_usize;
        let mut convolver = LinearPhaseConvolver::new(block_size);
        let mut out_l = Vec::new();
        let mut sample_idx = 0_usize;

        let mut push_sine_block = |conv: &mut LinearPhaseConvolver, cursor: &mut usize| {
            let mut l: Vec<f32> = (0..block_size)
                .map(|i| 0.20 * (2.0 * PI * 1000.0 * (*cursor + i) as f32 / sample_rate).sin())
                .collect();
            let mut r = l.clone();
            *cursor += block_size;
            conv.process_stereo(&mut l, &mut r);
            out_l.extend_from_slice(&l);
        };

        for _ in 0..8 {
            push_sine_block(&mut convolver, &mut sample_idx);
        }

        let mut final_params = EqParameters::default();
        for step in 1..=50 {
            let gain_db = (step as f32 / 50.0) * 12.0;
            final_params = bell_params(gain_db, 1000.0, 1.0, BandRouting::Stereo);
            convolver.request_kernel_update(
                &final_params,
                sample_rate,
                ProcessingMode::LinearPhaseLow,
            );
        }
        let target_gen = convolver.worker_next_generation();
        assert_eq!(target_gen, 50);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut settled_blocks = 0_usize;
        while settled_blocks < 10 {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for coalesced automation kernel"
            );
            push_sine_block(&mut convolver, &mut sample_idx);
            if convolver.worker_last_polled_generation() >= target_gen
                && !convolver.is_crossfade_pending()
            {
                settled_blocks += 1;
            } else {
                std::thread::yield_now();
            }
        }

        assert_eq!(convolver.worker_last_published_generation(), target_gen);
        let synth_count = convolver.worker_synthesis_count();
        assert!(
            (1..50).contains(&synth_count),
            "expected rapid requests to coalesce (< 50 syntheses), got {synth_count}"
        );

        assert!(out_l.iter().all(|s| s.is_finite()));
        let max_delta = out_l
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_delta < 0.35,
            "coalesced automation update caused discontinuity: max_delta={max_delta}"
        );

        let coeffs = FilterSpecification::from_band(&final_params.bands[0], sample_rate).compute();
        let ref_db = biquad_response_at_frequency(&coeffs, 1000.0, sample_rate);
        let in_rms = 0.20 * std::f32::consts::FRAC_1_SQRT_2;
        let out_rms = rms(&out_l[out_l.len() - 1024..]);
        let measured_db = 20.0 * (out_rms / in_rms).log10();
        assert!(
            (measured_db - ref_db).abs() < 0.3,
            "expected ~{ref_db:.3} dB, got {measured_db:.3} dB"
        );
    }

    #[test]
    fn test_async_kernel_discards_stale_mode_result() {
        use std::time::{Duration, Instant};

        let sample_rate = 48_000.0_f32;
        let mut convolver = LinearPhaseConvolver::new(256);
        let boosted = bell_params(12.0, 1000.0, 1.0, BandRouting::Stereo);

        convolver.request_kernel_update(&boosted, sample_rate, ProcessingMode::LinearPhaseLow);

        let deadline = Instant::now() + Duration::from_secs(5);
        while convolver.worker_last_published_generation() < 1 {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for worker to complete stale bank"
            );
            std::thread::yield_now();
        }

        // Switching mode synchronously invalidates generation 1 and changes current_mode.
        convolver.update_kernel(
            &EqParameters::default(),
            sample_rate,
            ProcessingMode::LinearPhaseHigh,
        );
        assert!(!convolver.poll_async_kernel());

        let (left, right) = run_impulse_stereo(&mut convolver, 6144, 256);
        for (idx, (&l, &r)) in left.iter().zip(right.iter()).enumerate() {
            if idx == 2048 {
                assert!(
                    (l - 1.0).abs() < 1e-5 && (r - 1.0).abs() < 1e-5,
                    "expected unity impulse at 2048, got l={l}, r={r}"
                );
            } else {
                assert!(
                    l.abs() < 1e-5 && r.abs() < 1e-5,
                    "expected ~0.0 at {idx}, got l={l}, r={r}"
                );
            }
        }
    }

    #[test]
    fn test_async_kernel_worker_drops_cleanly() {
        let idle = LinearPhaseConvolver::new(256);
        drop(idle);

        for _ in 0..8 {
            let mut active = LinearPhaseConvolver::new(256);
            let params = bell_params(6.0, 1000.0, 1.0, BandRouting::Stereo);
            active.request_kernel_update(&params, 48_000.0, ProcessingMode::LinearPhaseLow);
            drop(active);
        }
    }
}
