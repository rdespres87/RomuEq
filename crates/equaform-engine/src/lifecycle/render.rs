//! Planar DSP and Band Solo rendering helpers for the real-time audio path.

use super::DELAY_SAMPLES_PER_STAGE;
use equaform_common::{BandRouting, EqParameters, MAX_BANDS};
use equaform_dsp::LinearPhaseConvolver;
use equaform_dsp::coefficients::BiquadCoefficients;
use equaform_dsp::{
    lr_to_ms_in_place, ms_to_lr_in_place, process_mono_in_place, process_mono_in_place_ramped,
    process_stereo_planar_in_place,
};

/// Planar buffer dimensions for a single audio block.
#[derive(Clone, Copy, Debug)]
pub(super) struct BlockLayout {
    pub(super) max_block: usize,
    pub(super) channels: usize,
    pub(super) block_size: usize,
}

/// Linear crossfade ramp parameters for a single audio block.
#[derive(Clone, Copy, Debug)]
pub(super) struct RampParams {
    pub(super) start: f32,
    pub(super) target: f32,
    pub(super) step: f32,
    pub(super) ramping: bool,
}

impl RampParams {
    /// Advance the ramp mix across `block_size` samples, clamped to `target`.
    #[inline]
    pub(super) fn next_mix(self, block_size: usize) -> f32 {
        if !self.ramping {
            return self.start;
        }
        let delta = self.step * block_size as f32;
        if self.start < self.target {
            (self.start + delta).min(self.target)
        } else {
            (self.start - delta).max(self.target)
        }
    }
}

/// Borrowed planar slices for stereo Band Solo audition rendering.
pub(super) struct StereoSoloIo<'a> {
    pub(super) out_l: &'a mut [f32],
    pub(super) out_r: &'a mut [f32],
    pub(super) dry_l: &'a [f32],
    pub(super) dry_r: &'a [f32],
    pub(super) scratch_l: &'a mut [f32],
    pub(super) scratch_r: &'a mut [f32],
}

/// Borrowed slices and channel metadata for mono/multi-channel Band Solo rendering.
pub(super) struct MonoSoloIo<'a> {
    pub(super) out: &'a mut [f32],
    pub(super) dry: &'a [f32],
    pub(super) scratch: &'a mut [f32],
    pub(super) ch_idx: usize,
    pub(super) channels: usize,
}

/// Borrow left/right (or mid/side) 4-sample biquad delay slices for a stereo band.
#[inline]
fn band_delay_pair_mut(
    delay_line: &mut [f32],
    band_idx: usize,
) -> Option<(&mut [f32; 4], &mut [f32; 4])> {
    let offset = band_idx * 2 * DELAY_SAMPLES_PER_STAGE;
    let pair = delay_line.get_mut(offset..offset + 2 * DELAY_SAMPLES_PER_STAGE)?;
    let (delay_l_slice, delay_r_slice) = pair.split_at_mut(DELAY_SAMPLES_PER_STAGE);
    let delay_l = <&mut [f32; 4]>::try_from(delay_l_slice).ok()?;
    let delay_r = <&mut [f32; 4]>::try_from(delay_r_slice).ok()?;
    Some((delay_l, delay_r))
}

/// Accumulate `src` into `dst` sample-by-sample.
#[inline]
fn accumulate_slice(dst: &mut [f32], src: &[f32]) {
    for (d, &s) in dst.iter_mut().zip(src.iter()) {
        *d += s;
    }
}

/// Process planar audio in place using the UPOLS linear-phase convolver.
pub(super) fn process_linear_phase_planar(
    convolver: &mut LinearPhaseConvolver,
    buffer: &mut [f32],
    dry_buffer: &[f32],
    layout: BlockLayout,
) {
    if layout.channels == 1 {
        convolver.process_mono(&mut buffer[..layout.block_size]);
        return;
    }

    let (left_buf, right_buf) = buffer.split_at_mut(layout.max_block);
    convolver.process_stereo(
        &mut left_buf[..layout.block_size],
        &mut right_buf[..layout.block_size],
    );

    for ch in 2..layout.channels {
        let offset = ch * layout.max_block;
        buffer[offset..offset + layout.block_size]
            .copy_from_slice(&dry_buffer[offset..offset + layout.block_size]);
    }
}

/// Bit-exact coefficient comparison to detect steady-state bands.
#[inline]
fn coeffs_eq(a: &BiquadCoefficients, b: &BiquadCoefficients) -> bool {
    a.b0.to_bits() == b.b0.to_bits()
        && a.b1.to_bits() == b.b1.to_bits()
        && a.b2.to_bits() == b.b2.to_bits()
        && a.a1.to_bits() == b.a1.to_bits()
        && a.a2.to_bits() == b.a2.to_bits()
}

/// Zero a band's delay-line state across all active channels.
#[inline]
fn zero_band_delay(delay_line: &mut [f32], band_idx: usize, channels: usize) {
    let dl_base = band_idx * channels * DELAY_SAMPLES_PER_STAGE;
    let dl_len = channels * DELAY_SAMPLES_PER_STAGE;
    if let Some(slice) = delay_line.get_mut(dl_base..dl_base + dl_len) {
        slice.fill(0.0);
    }
}

/// Per-lane start/target biquad coefficients for a single band and block.
#[derive(Clone, Copy)]
struct BandStep {
    start_l: BiquadCoefficients,
    target_l: BiquadCoefficients,
    start_r: BiquadCoefficients,
    target_r: BiquadCoefficients,
    routing: BandRouting,
}

/// Resolve per-lane `(left/mid, right/side)` target coefficients for a band's enable and routing state.
#[inline]
fn target_pair_for_routing(
    enabled: bool,
    coeffs: BiquadCoefficients,
    routing: BandRouting,
) -> (BiquadCoefficients, BiquadCoefficients) {
    let identity = BiquadCoefficients::default();
    if !enabled {
        return (identity, identity);
    }
    match routing {
        BandRouting::Stereo => (coeffs, coeffs),
        BandRouting::Left | BandRouting::Mid => (coeffs, identity),
        BandRouting::Right | BandRouting::Side => (identity, coeffs),
    }
}

/// Process a single channel lane when either `start` or `target` differs from identity.
#[inline]
fn process_lane_if_active(
    samples: &mut [f32],
    start: &BiquadCoefficients,
    target: &BiquadCoefficients,
    ramp_samples: usize,
    delay: &mut [f32; 4],
) {
    let identity = BiquadCoefficients::default();
    if coeffs_eq(start, &identity) && coeffs_eq(target, &identity) {
        return;
    }
    if coeffs_eq(start, target) {
        process_mono_in_place(samples, target, delay);
    } else {
        process_mono_in_place_ramped(samples, start, target, ramp_samples, delay);
    }
}

/// Prepare per-lane `(start, target)` coefficients for one band, converting or flushing
/// delay lines on routing/enable transitions so stale filter state never leaks into audio.
#[inline]
fn prepare_band_step(
    band_idx: usize,
    block_coeffs: &[BiquadCoefficients; MAX_BANDS],
    eq_params: &EqParameters,
    delay_line: &mut [f32],
    channels: usize,
    cache: &mut super::CoefficientCache,
) -> Option<BandStep> {
    let enabled = eq_params.bands[band_idx].enabled;
    let routing = eq_params.band_routing(band_idx);
    let identity = BiquadCoefficients::default();

    if routing != cache.prev_routing[band_idx] {
        let was_ms = matches!(
            cache.prev_routing[band_idx],
            BandRouting::Mid | BandRouting::Side
        );
        let is_ms = matches!(routing, BandRouting::Mid | BandRouting::Side);
        if channels == 2 && was_ms != is_ms {
            if let Some((delay_a, delay_b)) = band_delay_pair_mut(delay_line, band_idx) {
                if is_ms {
                    lr_to_ms_in_place(delay_a, delay_b);
                } else {
                    ms_to_lr_in_place(delay_a, delay_b);
                }
            }
        } else if channels != 2 {
            zero_band_delay(delay_line, band_idx, channels);
            cache.prev_coeffs_l[band_idx] = identity;
            cache.prev_coeffs_r[band_idx] = identity;
        }
        cache.prev_routing[band_idx] = routing;
    }

    if enabled
        && !cache.prev_enabled[band_idx]
        && coeffs_eq(&cache.prev_coeffs_l[band_idx], &identity)
        && coeffs_eq(&cache.prev_coeffs_r[band_idx], &identity)
    {
        zero_band_delay(delay_line, band_idx, channels);
    }

    let (target_l, target_r) = target_pair_for_routing(enabled, block_coeffs[band_idx], routing);
    let start_l = cache.prev_coeffs_l[band_idx];
    let start_r = cache.prev_coeffs_r[band_idx];

    if !enabled && coeffs_eq(&start_l, &identity) && coeffs_eq(&start_r, &identity) {
        cache.prev_enabled[band_idx] = false;
        cache.prev_coeffs_l[band_idx] = identity;
        cache.prev_coeffs_r[band_idx] = identity;
        return None;
    }

    cache.prev_coeffs_l[band_idx] = target_l;
    cache.prev_coeffs_r[band_idx] = target_r;
    cache.prev_enabled[band_idx] = enabled;

    Some(BandStep {
        start_l,
        target_l,
        start_r,
        target_r,
        routing,
    })
}

/// Borrowed state for minimum-phase planar biquad processing.
pub(super) struct MinimumPhaseCtx<'a> {
    pub(super) block_coeffs: &'a [BiquadCoefficients; MAX_BANDS],
    pub(super) eq_params: &'a EqParameters,
    pub(super) delay_line: &'a mut [f32],
    pub(super) cache: &'a mut super::CoefficientCache,
    pub(super) sample_rate: f32,
    pub(super) is_first_block: bool,
}

/// Process planar audio in place using the minimum-phase biquad cascade.
pub(super) fn process_minimum_phase_planar(
    buffer: &mut [f32],
    layout: BlockLayout,
    ctx: MinimumPhaseCtx<'_>,
) {
    let MinimumPhaseCtx {
        block_coeffs,
        eq_params,
        delay_line,
        cache,
        sample_rate,
        is_first_block,
    } = ctx;

    if is_first_block {
        for (band_idx, &coeffs) in block_coeffs.iter().enumerate() {
            let enabled = eq_params.bands[band_idx].enabled;
            let routing = eq_params.band_routing(band_idx);
            let (tl, tr) = target_pair_for_routing(enabled, coeffs, routing);
            cache.prev_enabled[band_idx] = enabled;
            cache.prev_routing[band_idx] = routing;
            cache.prev_coeffs_l[band_idx] = tl;
            cache.prev_coeffs_r[band_idx] = tr;
        }
    }

    let ramp_samples = layout
        .block_size
        .min((0.005 * sample_rate.max(1.0)).round().max(1.0) as usize);

    let mut band_steps: [Option<BandStep>; MAX_BANDS] = [None; MAX_BANDS];
    for (band_idx, slot) in band_steps.iter_mut().enumerate() {
        *slot = prepare_band_step(
            band_idx,
            block_coeffs,
            eq_params,
            delay_line,
            layout.channels,
            cache,
        );
    }

    if layout.channels == 2 {
        let (left_buf, right_buf) = buffer.split_at_mut(layout.max_block);
        let left = &mut left_buf[..layout.block_size];
        let right = &mut right_buf[..layout.block_size];
        process_minimum_phase_stereo(left, right, &band_steps, ramp_samples, delay_line);
        return;
    }

    process_minimum_phase_channels(buffer, layout, &band_steps, ramp_samples, delay_line);
}

/// Process stereo minimum-phase bands (L/R pass followed by optional M/S pass).
fn process_minimum_phase_stereo(
    left: &mut [f32],
    right: &mut [f32],
    band_steps: &[Option<BandStep>; MAX_BANDS],
    ramp_samples: usize,
    delay_line: &mut [f32],
) {
    let mut has_ms_band = false;
    for (band_idx, step) in band_steps.iter().enumerate() {
        let Some(s) = step else {
            continue;
        };
        if matches!(s.routing, BandRouting::Mid | BandRouting::Side) {
            has_ms_band = true;
            continue;
        }
        if let Some((delay_l, delay_r)) = band_delay_pair_mut(delay_line, band_idx) {
            if coeffs_eq(&s.start_l, &s.target_l)
                && coeffs_eq(&s.start_r, &s.target_r)
                && coeffs_eq(&s.target_l, &s.target_r)
            {
                process_stereo_planar_in_place(left, right, &s.target_l, delay_l, delay_r);
            } else {
                process_lane_if_active(left, &s.start_l, &s.target_l, ramp_samples, delay_l);
                process_lane_if_active(right, &s.start_r, &s.target_r, ramp_samples, delay_r);
            }
        }
    }

    if has_ms_band {
        lr_to_ms_in_place(left, right);
        process_minimum_phase_stereo_ms(left, right, band_steps, ramp_samples, delay_line);
        ms_to_lr_in_place(left, right);
    }
}

/// Process Mid and Side minimum-phase bands while `mid` / `side` are in M/S domain.
fn process_minimum_phase_stereo_ms(
    mid: &mut [f32],
    side: &mut [f32],
    band_steps: &[Option<BandStep>; MAX_BANDS],
    ramp_samples: usize,
    delay_line: &mut [f32],
) {
    for (band_idx, step) in band_steps.iter().enumerate() {
        let Some(s) = step else {
            continue;
        };
        if !matches!(s.routing, BandRouting::Mid | BandRouting::Side) {
            continue;
        }
        if let Some((delay_m, delay_s)) = band_delay_pair_mut(delay_line, band_idx) {
            process_lane_if_active(mid, &s.start_l, &s.target_l, ramp_samples, delay_m);
            process_lane_if_active(side, &s.start_r, &s.target_r, ramp_samples, delay_s);
        }
    }
}

/// Process mono or multi-channel (>2) minimum-phase bands independently per channel.
fn process_minimum_phase_channels(
    buffer: &mut [f32],
    layout: BlockLayout,
    band_steps: &[Option<BandStep>; MAX_BANDS],
    ramp_samples: usize,
    delay_line: &mut [f32],
) {
    for ch_idx in 0..layout.channels {
        let buffer_offset = ch_idx * layout.max_block;
        let samples = &mut buffer[buffer_offset..buffer_offset + layout.block_size];
        for (band_idx, step) in band_steps.iter().enumerate() {
            let Some(s) = step else {
                continue;
            };
            let delay_offset = band_idx * layout.channels * DELAY_SAMPLES_PER_STAGE
                + ch_idx * DELAY_SAMPLES_PER_STAGE;
            let delay_slice = &mut delay_line[delay_offset..delay_offset + DELAY_SAMPLES_PER_STAGE];
            if let Ok(delay) = <&mut [f32; 4]>::try_from(delay_slice) {
                process_lane_if_active(samples, &s.start_l, &s.target_l, ramp_samples, delay);
            }
        }
    }
}

/// Target `(left/mid, right/side)` solo audition gains for a band's routing.
#[inline]
fn solo_target_gains(routing: BandRouting) -> (f32, f32) {
    match routing {
        BandRouting::Stereo => (1.0, 1.0),
        BandRouting::Left | BandRouting::Mid => (1.0, 0.0),
        BandRouting::Right | BandRouting::Side => (0.0, 1.0),
    }
}

/// Accumulate `src` into `dst` with a per-sample linear gain ramp over `ramp_samples`.
#[inline]
fn accumulate_slice_ramped(
    dst: &mut [f32],
    src: &[f32],
    start_gain: f32,
    target_gain: f32,
    ramp_samples: usize,
) {
    if (start_gain - target_gain).abs() <= f32::EPSILON {
        if (target_gain - 1.0).abs() <= f32::EPSILON {
            accumulate_slice(dst, src);
        } else if target_gain > f32::EPSILON {
            for (d, &s) in dst.iter_mut().zip(src.iter()) {
                *d += s * target_gain;
            }
        }
        return;
    }

    let steps = ramp_samples.clamp(1, dst.len().min(src.len()).max(1));
    let inv = 1.0 / steps as f32;
    let delta = (target_gain - start_gain) * inv;
    let mut gain = start_gain;
    for (i, (d, &s)) in dst.iter_mut().zip(src.iter()).enumerate() {
        if i < steps {
            gain += delta;
        } else {
            gain = target_gain;
        }
        *d += s * gain;
    }
}

/// Shared read-only configuration for stereo solo audition passes.
#[derive(Clone, Copy)]
pub(super) struct SoloPassConfig<'a> {
    pub active_mask: u8,
    pub solo_coeffs: &'a [BiquadCoefficients; MAX_BANDS],
    pub eq_params: &'a EqParameters,
    pub sample_rate: f32,
}

/// Render parallel stereo Band Solo audition into `io.out_l` / `io.out_r` from dry input.
pub(super) fn render_stereo_solo_audition(
    mut io: StereoSoloIo<'_>,
    config: SoloPassConfig<'_>,
    solo_delay_line: &mut [f32],
    routing_state: &mut super::SoloRoutingState,
) {
    io.out_l.fill(0.0);
    io.out_r.fill(0.0);

    let len = io.out_l.len().min(io.out_r.len());
    let ramp_samples = len.min((0.005 * config.sample_rate.max(1.0)).round().max(1.0) as usize);

    if !routing_state.initialized {
        for band_idx in 0..MAX_BANDS {
            let routing = config.eq_params.band_routing(band_idx);
            let (gl, gr) = solo_target_gains(routing);
            routing_state.prev_routing[band_idx] = routing;
            routing_state.prev_gain_l[band_idx] = gl;
            routing_state.prev_gain_r[band_idx] = gr;
        }
        routing_state.initialized = true;
    }

    let mut domain_switched = false;
    for band_idx in 0..MAX_BANDS {
        if (config.active_mask & (1 << band_idx)) == 0 {
            continue;
        }
        let routing = config.eq_params.band_routing(band_idx);
        if routing != routing_state.prev_routing[band_idx] {
            let was_ms = matches!(
                routing_state.prev_routing[band_idx],
                BandRouting::Mid | BandRouting::Side
            );
            let is_ms = matches!(routing, BandRouting::Mid | BandRouting::Side);
            if was_ms != is_ms {
                zero_band_delay(solo_delay_line, band_idx, 2);
                routing_state.prev_gain_l[band_idx] = 0.0;
                routing_state.prev_gain_r[band_idx] = 0.0;
                domain_switched = true;
            }
            routing_state.prev_routing[band_idx] = routing;
        }
    }

    // Pass 1: accumulate Mid/Side solo bands in M/S domain, then convert to L/R once.
    render_stereo_solo_ms_pass(
        &mut io,
        config,
        solo_delay_line,
        routing_state,
        ramp_samples,
    );

    // Pass 2: accumulate Stereo / Left / Right solo bands directly in L/R domain.
    render_stereo_solo_lr_pass(
        &mut io,
        config,
        solo_delay_line,
        routing_state,
        ramp_samples,
    );

    if domain_switched {
        let decay = (-1.0_f32 / (0.002 * config.sample_rate.max(1.0))).exp();
        let mut tl = routing_state.last_out[0];
        let mut tr = routing_state.last_out[1];
        for (ol, or) in io.out_l.iter_mut().zip(io.out_r.iter_mut()) {
            tl *= decay;
            tr *= decay;
            *ol += tl;
            *or += tr;
        }
    }

    if let (Some(&last_l), Some(&last_r)) = (io.out_l.last(), io.out_r.last()) {
        routing_state.last_out = [last_l, last_r];
    }
}

/// Accumulate Mid/Side solo bands in M/S domain and convert to L/R once.
fn render_stereo_solo_ms_pass(
    io: &mut StereoSoloIo<'_>,
    config: SoloPassConfig<'_>,
    solo_delay_line: &mut [f32],
    routing_state: &mut super::SoloRoutingState,
    ramp_samples: usize,
) {
    const INV_SQRT_2: f32 = std::f32::consts::FRAC_1_SQRT_2;

    let mut has_ms_solo = false;
    for (band_idx, coeffs) in config.solo_coeffs.iter().enumerate() {
        if (config.active_mask & (1 << band_idx)) == 0 {
            continue;
        }
        let routing = config.eq_params.band_routing(band_idx);
        if !matches!(routing, BandRouting::Mid | BandRouting::Side) {
            continue;
        }
        has_ms_solo = true;
        let (target_m, target_s) = solo_target_gains(routing);
        let start_m = routing_state.prev_gain_l[band_idx];
        let start_s = routing_state.prev_gain_r[band_idx];

        if let Some((delay_m, delay_s)) = band_delay_pair_mut(solo_delay_line, band_idx) {
            if start_m > f32::EPSILON || target_m > f32::EPSILON {
                if start_m <= f32::EPSILON {
                    delay_m.fill(0.0);
                }
                for ((dst, &dl), &dr) in io
                    .scratch_l
                    .iter_mut()
                    .zip(io.dry_l.iter())
                    .zip(io.dry_r.iter())
                {
                    *dst = (dl + dr) * INV_SQRT_2;
                }
                process_mono_in_place(io.scratch_l, coeffs, delay_m);
                accumulate_slice_ramped(io.out_l, io.scratch_l, start_m, target_m, ramp_samples);
            }
            if start_s > f32::EPSILON || target_s > f32::EPSILON {
                if start_s <= f32::EPSILON {
                    delay_s.fill(0.0);
                }
                for ((dst, &dl), &dr) in io
                    .scratch_r
                    .iter_mut()
                    .zip(io.dry_l.iter())
                    .zip(io.dry_r.iter())
                {
                    *dst = (dl - dr) * INV_SQRT_2;
                }
                process_mono_in_place(io.scratch_r, coeffs, delay_s);
                accumulate_slice_ramped(io.out_r, io.scratch_r, start_s, target_s, ramp_samples);
            }
        }

        routing_state.prev_gain_l[band_idx] = target_m;
        routing_state.prev_gain_r[band_idx] = target_s;
    }

    if has_ms_solo {
        ms_to_lr_in_place(io.out_l, io.out_r);
    }
}

/// Accumulate Stereo, Left, and Right solo bands directly in L/R domain.
fn render_stereo_solo_lr_pass(
    io: &mut StereoSoloIo<'_>,
    config: SoloPassConfig<'_>,
    solo_delay_line: &mut [f32],
    routing_state: &mut super::SoloRoutingState,
    ramp_samples: usize,
) {
    for (band_idx, coeffs) in config.solo_coeffs.iter().enumerate() {
        if (config.active_mask & (1 << band_idx)) == 0 {
            continue;
        }
        let routing = config.eq_params.band_routing(band_idx);
        if matches!(routing, BandRouting::Mid | BandRouting::Side) {
            continue;
        }
        let (target_l, target_r) = solo_target_gains(routing);
        let start_l = routing_state.prev_gain_l[band_idx];
        let start_r = routing_state.prev_gain_r[band_idx];

        if let Some((delay_l, delay_r)) = band_delay_pair_mut(solo_delay_line, band_idx) {
            if start_l > f32::EPSILON || target_l > f32::EPSILON {
                if start_l <= f32::EPSILON {
                    delay_l.fill(0.0);
                }
                io.scratch_l.copy_from_slice(io.dry_l);
                process_mono_in_place(io.scratch_l, coeffs, delay_l);
                accumulate_slice_ramped(io.out_l, io.scratch_l, start_l, target_l, ramp_samples);
            }
            if start_r > f32::EPSILON || target_r > f32::EPSILON {
                if start_r <= f32::EPSILON {
                    delay_r.fill(0.0);
                }
                io.scratch_r.copy_from_slice(io.dry_r);
                process_mono_in_place(io.scratch_r, coeffs, delay_r);
                accumulate_slice_ramped(io.out_r, io.scratch_r, start_r, target_r, ramp_samples);
            }
        }

        routing_state.prev_gain_l[band_idx] = target_l;
        routing_state.prev_gain_r[band_idx] = target_r;
    }
}

/// Render parallel mono/multi-channel Band Solo audition into `io.out` from `io.dry`.
pub(super) fn render_mono_solo_audition(
    io: MonoSoloIo<'_>,
    active_mask: u8,
    solo_coeffs: &[BiquadCoefficients; MAX_BANDS],
    solo_delay_line: &mut [f32],
) {
    io.out.fill(0.0);
    for (band_idx, coeffs) in solo_coeffs.iter().enumerate() {
        if (active_mask & (1 << band_idx)) == 0 {
            continue;
        }
        io.scratch.copy_from_slice(io.dry);
        let delay_offset =
            band_idx * io.channels * DELAY_SAMPLES_PER_STAGE + io.ch_idx * DELAY_SAMPLES_PER_STAGE;
        let delay_slice =
            &mut solo_delay_line[delay_offset..delay_offset + DELAY_SAMPLES_PER_STAGE];
        if let Ok(delay) = <&mut [f32; 4]>::try_from(delay_slice) {
            process_mono_in_place(io.scratch, coeffs, delay);
        }
        accumulate_slice(io.out, io.scratch);
    }
}

/// Apply continuous equal-power sin/cos crossfade between `samples` (`mix == 0.0`) and
/// `other_samples` (`mix == 1.0`).
#[inline]
pub(super) fn apply_equal_power_crossfade(
    samples: &mut [f32],
    other_samples: &[f32],
    ramp: RampParams,
) {
    let mut mix = ramp.start;
    for (sample, &other) in samples.iter_mut().zip(other_samples.iter()) {
        let angle = mix * std::f32::consts::FRAC_PI_2;
        let primary_gain = angle.cos();
        let secondary_gain = angle.sin();
        *sample = *sample * primary_gain + other * secondary_gain;

        if mix < ramp.target {
            mix = (mix + ramp.step).min(ramp.target);
        } else if mix > ramp.target {
            mix = (mix - ramp.step).max(ramp.target);
        }
    }
}
