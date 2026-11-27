//! Planar DSP and Band Solo rendering helpers for the real-time audio path.

use super::DELAY_SAMPLES_PER_STAGE;
use equaform_common::{BandRouting, EqParameters, MAX_BANDS};
use equaform_dsp::LinearPhaseConvolver;
use equaform_dsp::coefficients::BiquadCoefficients;
use equaform_dsp::{
    lr_to_ms_in_place, ms_to_lr_in_place, process_mono_in_place, process_stereo_planar_in_place,
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

/// Process planar audio in place using the minimum-phase biquad cascade.
pub(super) fn process_minimum_phase_planar(
    buffer: &mut [f32],
    layout: BlockLayout,
    block_coeffs: &[BiquadCoefficients; MAX_BANDS],
    eq_params: &EqParameters,
    delay_line: &mut [f32],
) {
    if layout.channels == 2 {
        let (left_buf, right_buf) = buffer.split_at_mut(layout.max_block);
        let left = &mut left_buf[..layout.block_size];
        let right = &mut right_buf[..layout.block_size];
        process_minimum_phase_stereo(left, right, block_coeffs, eq_params, delay_line);
        return;
    }

    process_minimum_phase_channels(buffer, layout, block_coeffs, eq_params, delay_line);
}

/// Process stereo minimum-phase bands (L/R pass followed by optional M/S pass).
fn process_minimum_phase_stereo(
    left: &mut [f32],
    right: &mut [f32],
    block_coeffs: &[BiquadCoefficients; MAX_BANDS],
    eq_params: &EqParameters,
    delay_line: &mut [f32],
) {
    let mut has_ms_band = false;
    for (band_idx, coeffs) in block_coeffs.iter().enumerate() {
        if !eq_params.bands[band_idx].enabled {
            continue;
        }
        let routing = eq_params.band_routing(band_idx);
        if matches!(routing, BandRouting::Mid | BandRouting::Side) {
            has_ms_band = true;
            continue;
        }
        if let Some((delay_l, delay_r)) = band_delay_pair_mut(delay_line, band_idx) {
            match routing {
                BandRouting::Stereo => {
                    process_stereo_planar_in_place(left, right, coeffs, delay_l, delay_r);
                }
                BandRouting::Left => {
                    process_mono_in_place(left, coeffs, delay_l);
                }
                BandRouting::Right => {
                    process_mono_in_place(right, coeffs, delay_r);
                }
                BandRouting::Mid | BandRouting::Side => {}
            }
        }
    }

    if has_ms_band {
        lr_to_ms_in_place(left, right);
        process_minimum_phase_stereo_ms(left, right, block_coeffs, eq_params, delay_line);
        ms_to_lr_in_place(left, right);
    }
}

/// Process Mid and Side minimum-phase bands while `mid` / `side` are in M/S domain.
fn process_minimum_phase_stereo_ms(
    mid: &mut [f32],
    side: &mut [f32],
    block_coeffs: &[BiquadCoefficients; MAX_BANDS],
    eq_params: &EqParameters,
    delay_line: &mut [f32],
) {
    for (band_idx, coeffs) in block_coeffs.iter().enumerate() {
        if !eq_params.bands[band_idx].enabled {
            continue;
        }
        let routing = eq_params.band_routing(band_idx);
        if !matches!(routing, BandRouting::Mid | BandRouting::Side) {
            continue;
        }
        if let Some((delay_m, delay_s)) = band_delay_pair_mut(delay_line, band_idx) {
            match routing {
                BandRouting::Mid => process_mono_in_place(mid, coeffs, delay_m),
                BandRouting::Side => process_mono_in_place(side, coeffs, delay_s),
                _ => {}
            }
        }
    }
}

/// Process mono or multi-channel (>2) minimum-phase bands independently per channel.
fn process_minimum_phase_channels(
    buffer: &mut [f32],
    layout: BlockLayout,
    block_coeffs: &[BiquadCoefficients; MAX_BANDS],
    eq_params: &EqParameters,
    delay_line: &mut [f32],
) {
    for ch_idx in 0..layout.channels {
        let buffer_offset = ch_idx * layout.max_block;
        let samples = &mut buffer[buffer_offset..buffer_offset + layout.block_size];
        for (band_idx, coeffs) in block_coeffs.iter().enumerate() {
            if !eq_params.bands[band_idx].enabled {
                continue;
            }
            let delay_offset = band_idx * layout.channels * DELAY_SAMPLES_PER_STAGE
                + ch_idx * DELAY_SAMPLES_PER_STAGE;
            let delay_slice = &mut delay_line[delay_offset..delay_offset + DELAY_SAMPLES_PER_STAGE];
            if let Ok(delay) = <&mut [f32; 4]>::try_from(delay_slice) {
                process_mono_in_place(samples, coeffs, delay);
            }
        }
    }
}

/// Render parallel stereo Band Solo audition into `io.out_l` / `io.out_r` from dry input.
pub(super) fn render_stereo_solo_audition(
    mut io: StereoSoloIo<'_>,
    active_mask: u8,
    solo_coeffs: &[BiquadCoefficients; MAX_BANDS],
    eq_params: &EqParameters,
    solo_delay_line: &mut [f32],
) {
    io.out_l.fill(0.0);
    io.out_r.fill(0.0);

    // Pass 1: accumulate Mid/Side solo bands in M/S domain, then convert to L/R once.
    render_stereo_solo_ms_pass(
        &mut io,
        active_mask,
        solo_coeffs,
        eq_params,
        solo_delay_line,
    );

    // Pass 2: accumulate Stereo / Left / Right solo bands directly in L/R domain.
    render_stereo_solo_lr_pass(
        &mut io,
        active_mask,
        solo_coeffs,
        eq_params,
        solo_delay_line,
    );
}

/// Accumulate Mid/Side solo bands in M/S domain and convert to L/R once.
fn render_stereo_solo_ms_pass(
    io: &mut StereoSoloIo<'_>,
    active_mask: u8,
    solo_coeffs: &[BiquadCoefficients; MAX_BANDS],
    eq_params: &EqParameters,
    solo_delay_line: &mut [f32],
) {
    const INV_SQRT_2: f32 = std::f32::consts::FRAC_1_SQRT_2;

    let mut has_ms_solo = false;
    for (band_idx, coeffs) in solo_coeffs.iter().enumerate() {
        if (active_mask & (1 << band_idx)) == 0 {
            continue;
        }
        let routing = eq_params.band_routing(band_idx);
        if !matches!(routing, BandRouting::Mid | BandRouting::Side) {
            continue;
        }
        has_ms_solo = true;
        if let Some((delay_l, delay_r)) = band_delay_pair_mut(solo_delay_line, band_idx) {
            match routing {
                BandRouting::Mid => {
                    for ((dst, &dl), &dr) in io
                        .scratch_l
                        .iter_mut()
                        .zip(io.dry_l.iter())
                        .zip(io.dry_r.iter())
                    {
                        *dst = (dl + dr) * INV_SQRT_2;
                    }
                    process_mono_in_place(io.scratch_l, coeffs, delay_l);
                    accumulate_slice(io.out_l, io.scratch_l);
                }
                BandRouting::Side => {
                    for ((dst, &dl), &dr) in io
                        .scratch_r
                        .iter_mut()
                        .zip(io.dry_l.iter())
                        .zip(io.dry_r.iter())
                    {
                        *dst = (dl - dr) * INV_SQRT_2;
                    }
                    process_mono_in_place(io.scratch_r, coeffs, delay_r);
                    accumulate_slice(io.out_r, io.scratch_r);
                }
                _ => {}
            }
        }
    }

    if has_ms_solo {
        ms_to_lr_in_place(io.out_l, io.out_r);
    }
}

/// Accumulate Stereo, Left, and Right solo bands directly in L/R domain.
fn render_stereo_solo_lr_pass(
    io: &mut StereoSoloIo<'_>,
    active_mask: u8,
    solo_coeffs: &[BiquadCoefficients; MAX_BANDS],
    eq_params: &EqParameters,
    solo_delay_line: &mut [f32],
) {
    for (band_idx, coeffs) in solo_coeffs.iter().enumerate() {
        if (active_mask & (1 << band_idx)) == 0 {
            continue;
        }
        let routing = eq_params.band_routing(band_idx);
        if matches!(routing, BandRouting::Mid | BandRouting::Side) {
            continue;
        }
        if let Some((delay_l, delay_r)) = band_delay_pair_mut(solo_delay_line, band_idx) {
            match routing {
                BandRouting::Stereo => {
                    io.scratch_l.copy_from_slice(io.dry_l);
                    io.scratch_r.copy_from_slice(io.dry_r);
                    process_stereo_planar_in_place(
                        io.scratch_l,
                        io.scratch_r,
                        coeffs,
                        delay_l,
                        delay_r,
                    );
                    accumulate_slice(io.out_l, io.scratch_l);
                    accumulate_slice(io.out_r, io.scratch_r);
                }
                BandRouting::Left => {
                    io.scratch_l.copy_from_slice(io.dry_l);
                    process_mono_in_place(io.scratch_l, coeffs, delay_l);
                    accumulate_slice(io.out_l, io.scratch_l);
                }
                BandRouting::Right => {
                    io.scratch_r.copy_from_slice(io.dry_r);
                    process_mono_in_place(io.scratch_r, coeffs, delay_r);
                    accumulate_slice(io.out_r, io.scratch_r);
                }
                BandRouting::Mid | BandRouting::Side => {}
            }
        }
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
