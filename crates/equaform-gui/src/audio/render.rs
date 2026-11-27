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
    let frames = (data.len() / ctx.format.channels).min(MAX_SCRATCH_FRAMES);
    if frames == 0 {
        return;
    }
    if !ctx.telemetry.is_running.load(Ordering::Acquire) {
        data.fill(0.0);
        return;
    }

    if let Some(new_params) = ctx.param_consumer.read_latest_copy() {
        ctx.engine.set_parameters(new_params);
    }

    fill_input_block(frames, ctx);
    run_engine_chunks(frames, ctx);
    let (max_l, max_r) = interleave_with_mute_and_peaks(data, frames, ctx);
    push_telemetry(frames, max_l, max_r, ctx);
}

fn fill_input_block(frames: usize, ctx: &mut AudioRenderContext<'_>) {
    let mode = AudioSourceMode::from(ctx.telemetry.source_mode.load(Ordering::Relaxed));
    match mode {
        AudioSourceMode::TestSignal => {
            let pi2 = std::f64::consts::PI * 2.0;
            let sr = f64::from(ctx.format.sample_rate);
            for i in 0..frames {
                let t = *ctx.phase / sr;
                // Warm musical chord: A220 + C-sharp277 + E330 + A440
                let s = 0.14 * (t * 220.00 * pi2).sin()
                    + 0.10 * (t * 277.18 * pi2).sin()
                    + 0.08 * (t * 329.63 * pi2).sin()
                    + 0.06 * (t * 440.00 * pi2).sin();
                let sample = s as f32;
                ctx.scratch.left_in[i] = sample;
                ctx.scratch.right_in[i] = sample;
                *ctx.phase = (*ctx.phase + 1.0) % (sr * 60.0);
            }
        }
        AudioSourceMode::PassThrough => {
            ctx.drift.read_from(
                ctx.audio_consumer,
                ctx.scratch.left_in,
                ctx.scratch.right_in,
                frames,
            );
        }
    }
}

fn run_engine_chunks(frames: usize, ctx: &mut AudioRenderContext<'_>) {
    let block_size = ctx.format.block_size;
    let mut offset = 0;
    while offset < frames {
        let chunk = (frames - offset).min(block_size);
        let in_slices: [&[f32]; 2] = [
            &ctx.scratch.left_in[offset..offset + chunk],
            &ctx.scratch.right_in[offset..offset + chunk],
        ];
        let mut out_slices: [&mut [f32]; 2] = [
            &mut ctx.scratch.left_out[offset..offset + chunk],
            &mut ctx.scratch.right_out[offset..offset + chunk],
        ];
        if !ctx.engine.process(&in_slices, &mut out_slices) {
            out_slices[0].copy_from_slice(in_slices[0]);
            out_slices[1].copy_from_slice(in_slices[1]);
        }
        offset += chunk;
    }
}

fn interleave_with_mute_and_peaks(
    data: &mut [f32],
    frames: usize,
    ctx: &mut AudioRenderContext<'_>,
) -> (f32, f32) {
    let channels = ctx.format.channels;
    let target_mute = if ctx.telemetry.is_muted.load(Ordering::Acquire) {
        0.0f32
    } else {
        1.0f32
    };
    let mute_step = 1.0f32 / (0.005 * ctx.format.sample_rate).max(1.0);
    let mut max_l = 0.0f32;
    let mut max_r = 0.0f32;

    for i in 0..frames {
        if *ctx.mute_gain < target_mute {
            *ctx.mute_gain = (*ctx.mute_gain + mute_step).min(target_mute);
        } else if *ctx.mute_gain > target_mute {
            *ctx.mute_gain = (*ctx.mute_gain - mute_step).max(target_mute);
        }
        let g = *ctx.mute_gain;
        let l = soft_limit(ctx.scratch.left_out[i]) * g;
        let r = soft_limit(ctx.scratch.right_out[i]) * g;
        max_l = max_l.max(l.abs());
        max_r = max_r.max(r.abs());

        let out_idx = i * channels;
        if channels == 1 {
            data[out_idx] = (l + r) * 0.5;
        } else {
            data[out_idx] = l;
            data[out_idx + 1] = r;
            if channels >= 4 {
                data[out_idx + 2] = l;
                data[out_idx + 3] = r;
            } else if channels == 3 {
                data[out_idx + 2] = 0.0;
            }
            for c in 4..channels {
                data[out_idx + c] = 0.0;
            }
        }
    }
    (max_l, max_r)
}

fn push_telemetry(frames: usize, max_l: f32, max_r: f32, ctx: &AudioRenderContext<'_>) {
    ctx.telemetry
        .peak_l
        .store(max_l.to_bits(), Ordering::Relaxed);
    ctx.telemetry
        .peak_r
        .store(max_r.to_bits(), Ordering::Relaxed);

    let spectrum_samples = ctx.telemetry.spectrum_samples;
    let spec_len = spectrum_samples.len();
    let capture_count = frames.min(spec_len);
    let keep = spec_len - capture_count;
    for i in 0..keep {
        let prev = spectrum_samples[i + capture_count].load(Ordering::Relaxed);
        spectrum_samples[i].store(prev, Ordering::Relaxed);
    }
    for i in 0..capture_count {
        let idx = frames - capture_count + i;
        let val = (ctx.scratch.left_out[idx] + ctx.scratch.right_out[idx]) * 0.5;
        spectrum_samples[keep + i].store(val.to_bits(), Ordering::Relaxed);
    }
}
