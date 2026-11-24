//! Real-time output block processing, soft limiting, and telemetry updates.

use super::{AudioSourceMode, ContinuousPiDriftController, MAX_SCRATCH_FRAMES};
use equaform_common::EqParameters;
use equaform_common::audio_ring::AudioRingConsumer;
use equaform_common::triple_buffer::TripleBufferConsumer;
use equaform_engine::EngineLifecycle;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

#[inline]
pub fn soft_limit(x: f32) -> f32 {
    let abs_x = x.abs();
    if abs_x <= 0.85 {
        x
    } else {
        let sign = x.signum();
        sign * (0.85 + 0.15 * ((abs_x - 0.85) / 0.15).tanh())
    }
}

/// Immutable audio stream format parameters for a render callback.
#[derive(Clone, Copy, Debug)]
pub struct StreamFormatSpec {
    pub channels: usize,
    pub sample_rate: f32,
    pub block_size: usize,
}

/// Lock-free atomic state and metering sinks shared between audio and UI threads.
pub struct AudioTelemetry<'a> {
    pub is_running: &'a AtomicBool,
    pub is_muted: &'a AtomicBool,
    pub source_mode: &'a AtomicU8,
    pub peak_l: &'a AtomicU32,
    pub peak_r: &'a AtomicU32,
    pub spectrum_samples: &'a [AtomicU32],
}

/// Pre-allocated planar stereo scratch slices for real-time audio processing.
pub struct AudioScratchBuffers<'a> {
    pub left_in: &'a mut [f32],
    pub right_in: &'a mut [f32],
    pub left_out: &'a mut [f32],
    pub right_out: &'a mut [f32],
}

/// Execution context bundling pre-allocated buffers, drift control, atomic states, and DSP engine.
pub struct AudioRenderContext<'a> {
    pub format: StreamFormatSpec,
    pub engine: &'a mut EngineLifecycle,
    pub param_consumer: &'a mut TripleBufferConsumer<EqParameters>,
    pub audio_consumer: &'a AudioRingConsumer,
    pub drift: &'a mut ContinuousPiDriftController,
    pub telemetry: AudioTelemetry<'a>,
    pub phase: &'a mut f64,
    pub mute_gain: &'a mut f32,
    pub scratch: AudioScratchBuffers<'a>,
}

/// Real-time audio block processor: wait-free, zero allocation, zero mutex locks.
pub(super) fn process_output_block(data: &mut [f32], ctx: &mut AudioRenderContext<'_>) {
    let channels = ctx.format.channels;
    let sample_rate = ctx.format.sample_rate;
    let block_size = ctx.format.block_size;
    let engine = &mut *ctx.engine;
    let param_consumer = &mut *ctx.param_consumer;
    let audio_consumer = ctx.audio_consumer;
    let drift = &mut *ctx.drift;
    let is_running = ctx.telemetry.is_running;
    let is_muted = ctx.telemetry.is_muted;
    let source_mode = ctx.telemetry.source_mode;
    let peak_l = ctx.telemetry.peak_l;
    let peak_r = ctx.telemetry.peak_r;
    let spectrum_samples = ctx.telemetry.spectrum_samples;
    let phase = &mut *ctx.phase;
    let mute_gain = &mut *ctx.mute_gain;
    let left_in = &mut *ctx.scratch.left_in;
    let right_in = &mut *ctx.scratch.right_in;
    let left_out = &mut *ctx.scratch.left_out;
    let right_out = &mut *ctx.scratch.right_out;
    let frames = (data.len() / channels).min(MAX_SCRATCH_FRAMES);
    if frames == 0 {
        return;
    }

    if !is_running.load(Ordering::Acquire) {
        data.fill(0.0);
        return;
    }

    // 1. Check for pending parameter updates non-blocking via lock-free triple buffer
    if let Some(new_params) = param_consumer.read_latest_copy() {
        engine.set_parameters(new_params);
    }

    let mode = AudioSourceMode::from(source_mode.load(Ordering::Relaxed));

    // 2. Fetch input samples or generate musical test signal
    match mode {
        AudioSourceMode::TestSignal => {
            let pi2 = std::f64::consts::PI * 2.0;
            let sr = sample_rate as f64;
            for i in 0..frames {
                let t = *phase / sr;
                // Warm musical chord: A220 + C#277 + E330 + A440
                let s = 0.14 * (t * 220.00 * pi2).sin()
                    + 0.10 * (t * 277.18 * pi2).sin()
                    + 0.08 * (t * 329.63 * pi2).sin()
                    + 0.06 * (t * 440.00 * pi2).sin();

                let sample_f32 = s as f32;
                left_in[i] = sample_f32;
                right_in[i] = sample_f32;
                *phase = (*phase + 1.0) % (sr * 60.0);
            }
        }
        AudioSourceMode::PassThrough => {
            drift.read_from(audio_consumer, left_in, right_in, frames);
        }
    }

    // 3. Always process audio through the engine in blocks so bypass crossfade
    //    ramps smoothly and biquad delay lines remain warm across rapid toggles.
    let mut offset = 0;
    while offset < frames {
        let chunk = (frames - offset).min(block_size);
        let in_slices: [&[f32]; 2] = [
            &left_in[offset..offset + chunk],
            &right_in[offset..offset + chunk],
        ];
        let mut out_slices: [&mut [f32]; 2] = [
            &mut left_out[offset..offset + chunk],
            &mut right_out[offset..offset + chunk],
        ];

        let success = engine.process(&in_slices, &mut out_slices);
        if !success {
            // Strict deterministic fallback on processing error
            out_slices[0].copy_from_slice(in_slices[0]);
            out_slices[1].copy_from_slice(in_slices[1]);
        }
        offset += chunk;
    }

    // 4. Multi-channel hardware interleaving, smooth 5 ms mute ramp, soft saturation, and zeroing unused channels
    let target_mute_gain = if is_muted.load(Ordering::Acquire) {
        0.0f32
    } else {
        1.0f32
    };
    let mute_step = 1.0f32 / (0.005 * sample_rate).max(1.0);

    let mut max_l = 0.0f32;
    let mut max_r = 0.0f32;

    for i in 0..frames {
        if *mute_gain < target_mute_gain {
            *mute_gain = (*mute_gain + mute_step).min(target_mute_gain);
        } else if *mute_gain > target_mute_gain {
            *mute_gain = (*mute_gain - mute_step).max(target_mute_gain);
        }
        let g = *mute_gain;
        let l = soft_limit(left_out[i]) * g;
        let r = soft_limit(right_out[i]) * g;
        max_l = max_l.max(l.abs());
        max_r = max_r.max(r.abs());

        let out_idx = i * channels;
        if channels == 1 {
            // Mono hardware output: combine Left and Right channels cleanly
            data[out_idx] = (l + r) * 0.5;
        } else {
            // Main hardware output: Left and Right (e.g. MacBook Pro speakers, headphones)
            data[out_idx] = l;
            data[out_idx + 1] = r;

            // Multi-channel interface: mirror stereo pair to channels 3/4 (headphone 2 / secondary out)
            if channels >= 4 {
                data[out_idx + 2] = l;
                data[out_idx + 3] = r;
            } else if channels == 3 {
                data[out_idx + 2] = 0.0;
            }
            // Explicitly zero-fill all remaining hardware channels (e.g. 5, 6 on 6-channel interfaces)
            for c in 4..channels {
                data[out_idx + c] = 0.0;
            }
        }
    }

    // 5. Update lock-free telemetry
    peak_l.store(max_l.to_bits(), Ordering::Relaxed);
    peak_r.store(max_r.to_bits(), Ordering::Relaxed);

    let spec_len = spectrum_samples.len();
    let capture_count = frames.min(spec_len);
    let keep = spec_len - capture_count;
    for i in 0..keep {
        let prev = spectrum_samples[i + capture_count].load(Ordering::Relaxed);
        spectrum_samples[i].store(prev, Ordering::Relaxed);
    }
    for i in 0..capture_count {
        let idx = frames - capture_count + i;
        let val = (left_out[idx] + right_out[idx]) * 0.5;
        spectrum_samples[keep + i].store(val.to_bits(), Ordering::Relaxed);
    }
}
