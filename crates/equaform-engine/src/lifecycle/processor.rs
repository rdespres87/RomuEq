//! Real-time audio processing, automation ingestion, DSP routing, and click-free transitions.

use super::{CROSSFADE_MS, DELAY_SAMPLES_PER_STAGE, EngineLifecycle, LifecycleState};
use equaform_analysis::AnalysisFrame;
use equaform_common::automation::AutomationValueProvider;
use equaform_common::{BandParameters, BandRouting, EqParameters, MAX_BANDS, ProcessingMode};
use equaform_dsp::LinearPhaseConvolver;
use equaform_dsp::coefficients::{BiquadCoefficients, FilterSpecification};
use equaform_dsp::{
    lr_to_ms_in_place, ms_to_lr_in_place, process_mono_in_place, process_stereo_planar_in_place,
};

impl EngineLifecycle {
    /// Create an automation value provider for the current block.
    fn create_automation_provider(&mut self, block_size: usize) -> Option<AutomationValueProvider> {
        let ingestor = self.automation_ingestor.as_mut()?;
        let batch = ingestor.process_block(block_size as u32);
        Some(ingestor.value_provider(&batch))
    }

    /// Process a single audio block.
    ///
    /// # Allocation
    ///
    /// **No allocation.** All data is copied to/from the flat pre-allocated buffer.
    /// DSP processing uses pre-allocated delay-line and UPOLS convolver state.
    pub fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]) -> bool {
        if self.state != LifecycleState::Ready {
            return false;
        }

        if inputs.len() != outputs.len() {
            return false;
        }

        let block_size = inputs.first().map(|s| s.len()).unwrap_or(0);

        if block_size == 0 || block_size > self.topology.max_block_size {
            return false;
        }

        if inputs.iter().any(|s| s.len() != block_size)
            || outputs.iter().any(|s| s.len() != block_size)
        {
            return false;
        }

        if inputs.len() != self.topology.channels {
            return false;
        }

        let automation_provider = self.create_automation_provider(block_size);

        for (ch, input_ch) in inputs.iter().enumerate().take(self.topology.channels) {
            let offset = ch * self.topology.max_block_size;
            self.buffer[offset..offset + block_size].copy_from_slice(input_ch);
        }

        self.process_dsp(block_size, &automation_provider);

        let frame = AnalysisFrame::compute_planar(
            &self.buffer,
            self.topology.max_block_size,
            self.topology.channels,
            self.topology.sample_rate,
            block_size,
            self.blocks_processed,
        );
        self.analysis_buffer.push(frame);

        for (ch, output_ch) in outputs.iter_mut().enumerate().take(self.topology.channels) {
            let offset = ch * self.topology.max_block_size;
            output_ch.copy_from_slice(&self.buffer[offset..offset + block_size]);
        }

        self.sample_offset += block_size as u64;
        self.blocks_processed += 1;
        true
    }

    /// Update bypass transition state (immediate on first block, click-free crossfade afterwards).
    #[inline]
    fn update_bypass_state(&mut self) {
        self.bypass
            .update(self.eq_params.global.bypass, self.blocks_processed == 0);
    }

    /// Apply per-block automation overrides to band coefficients and sync persistent caches.
    ///
    /// Returns `true` if any band parameter changed via automation in this block.
    fn apply_block_automation(
        &mut self,
        block_size: usize,
        sample_rate: f32,
        provider: &AutomationValueProvider,
        block_coeffs: &mut [BiquadCoefficients; MAX_BANDS],
    ) -> bool {
        let block_length = block_size as u32;
        let gain_min = BandParameters::gain_db_min();
        let gain_max = BandParameters::gain_db_max();
        let freq_min: f32 = 20.0;
        let freq_max: f32 = sample_rate / 2.0;

        let mut any_automated = false;
        for (band_idx, coeff_slot) in block_coeffs.iter_mut().enumerate() {
            let mut band_params = self.eq_params.bands[band_idx];
            if !band_params.enabled {
                continue;
            }

            let mut automated = false;
            if let Some(automation_value) = provider
                .last_value_in_block(band_params.gain_param_id(), block_length)
                .or_else(|| {
                    provider.value_at_sample(
                        band_params.gain_param_id(),
                        block_length.saturating_sub(1),
                        None,
                    )
                })
            {
                let gain_range = gain_max - gain_min;
                band_params.gain_db = gain_min + (automation_value + 1.0) * 0.5 * gain_range;
                automated = true;
            }

            if let Some(automation_value) = provider
                .last_value_in_block(band_params.frequency_param_id(), block_length)
                .or_else(|| {
                    provider.value_at_sample(
                        band_params.frequency_param_id(),
                        block_length.saturating_sub(1),
                        None,
                    )
                })
            {
                let t = automation_value * 0.5 + 0.5;
                band_params.frequency_hz = freq_min * (freq_max / freq_min).powf(t);
                automated = true;
            }

            if automated {
                *coeff_slot = FilterSpecification::from_band(&band_params, sample_rate).compute();
                self.eq_params.bands[band_idx] = band_params;
                self.coeff_cache.coeffs[band_idx] = *coeff_slot;
                any_automated = true;
            }
        }

        if any_automated && self.eq_params.global.auto_gain {
            self.update_auto_gain_db();
        }
        any_automated
    }

    /// Synchronize the linear-phase convolver and dry delay line for mode/parameter updates.
    fn sync_linear_phase_for_block(
        &mut self,
        target_mode: ProcessingMode,
        sample_rate: f32,
        is_first_block: bool,
        mode_changed: bool,
        coeff_was_dirty: bool,
        any_automated: bool,
    ) {
        if is_first_block {
            if target_mode.is_linear_phase() {
                self.linear_phase
                    .update_kernel(&self.eq_params, sample_rate, target_mode);
                self.linear_phase.reset();
                self.dry_delay_line.reset();
            }
            return;
        }

        if mode_changed {
            if self.mode_transition.use_tail_fade {
                if target_mode.is_linear_phase() {
                    self.linear_phase
                        .update_kernel(&self.eq_params, sample_rate, target_mode);
                    self.linear_phase.reset();
                    self.dry_delay_line.reset();
                } else {
                    self.delay_line.fill(0.0);
                    self.linear_phase.update_kernel(
                        &self.eq_params,
                        sample_rate,
                        ProcessingMode::MinimumPhase,
                    );
                }
            } else if target_mode.is_linear_phase() {
                // Keep IIR delay_line warm for outgoing MinimumPhase fade-out.
                self.linear_phase
                    .update_kernel(&self.eq_params, sample_rate, target_mode);
                self.linear_phase.reset();
                self.dry_delay_line.reset();
            } else {
                // Keep linear_phase warm in previous_mode for outgoing linear-phase fade-out.
                self.delay_line.fill(0.0);
            }
            return;
        }

        if (coeff_was_dirty || any_automated) && target_mode.is_linear_phase() {
            self.linear_phase
                .request_kernel_update(&self.eq_params, sample_rate, target_mode);
        }
    }

    /// Render the outgoing mode (or smooth tail-fade reference) into `mode_transition.scratch_buffer`.
    fn render_outgoing_mode(
        &mut self,
        block_size: usize,
        block_coeffs: &[BiquadCoefficients; MAX_BANDS],
        output_gain_linear: f32,
    ) {
        let max_block = self.topology.max_block_size;
        let channels = self.topology.channels;

        if self.mode_transition.use_tail_fade {
            for (ch, &tail) in self.mode_transition.prev_output[..channels]
                .iter()
                .enumerate()
            {
                let offset = ch * max_block;
                self.mode_transition.scratch_buffer[offset..offset + block_size].fill(tail);
            }
            return;
        }

        for ch in 0..channels {
            let offset = ch * max_block;
            self.mode_transition.scratch_buffer[offset..offset + block_size]
                .copy_from_slice(&self.buffer[offset..offset + block_size]);
        }

        if self.mode_transition.previous_mode == ProcessingMode::MinimumPhase {
            process_minimum_phase_planar(
                &mut self.mode_transition.scratch_buffer,
                max_block,
                channels,
                block_size,
                block_coeffs,
                &self.eq_params,
                &mut self.delay_line,
            );
        } else {
            process_linear_phase_planar(
                &mut self.linear_phase,
                &mut self.mode_transition.scratch_buffer,
                &self.bypass.dry_buffer,
                max_block,
                channels,
                block_size,
            );
        }

        if output_gain_linear != 1.0 {
            for ch in 0..channels {
                let offset = ch * max_block;
                self.mode_transition.scratch_buffer[offset..offset + block_size]
                    .iter_mut()
                    .for_each(|s| *s *= output_gain_linear);
            }
        }
    }

    /// Populate `bypass.dry_buffer` with either zero-latency or latency-aligned dry audio.
    fn populate_dry_buffer(&mut self, block_size: usize, target_mode: ProcessingMode) {
        let max_block = self.topology.max_block_size;
        let channels = self.topology.channels;

        if target_mode.is_linear_phase() {
            let delay = target_mode.latency_samples() as usize;
            for ch in 0..channels {
                let offset = ch * max_block;
                let input = &self.buffer[offset..offset + block_size];
                let dry = &mut self.bypass.dry_buffer[offset..offset + block_size];
                self.dry_delay_line.process_channel(ch, input, dry, delay);
            }
        } else {
            for ch in 0..channels {
                let offset = ch * max_block;
                self.bypass.dry_buffer[offset..offset + block_size]
                    .copy_from_slice(&self.buffer[offset..offset + block_size]);
            }
        }
    }

    /// Apply Band Solo audition into `self.buffer` from the latency-aligned `bypass.dry_buffer`.
    fn process_active_solo(
        &mut self,
        block_size: usize,
        active_solo_mask: u8,
        solo_ramping: bool,
        solo_start: f32,
        solo_target: f32,
        step: f32,
    ) {
        let max_block = self.topology.max_block_size;
        let channels = self.topology.channels;

        if channels == 2 {
            let (left_buf, right_buf) = self.buffer.split_at_mut(max_block);
            let left = &mut left_buf[..block_size];
            let right = &mut right_buf[..block_size];

            let (dry_left_buf, dry_right_buf) = self.bypass.dry_buffer.split_at_mut(max_block);
            let dry_left = &dry_left_buf[..block_size];
            let dry_right = &dry_right_buf[..block_size];

            let (scratch_l_buf, scratch_r_buf) = self.solo_scratch.split_at_mut(max_block);
            let scratch_l = &mut scratch_l_buf[..block_size];
            let scratch_r = &mut scratch_r_buf[..block_size];

            if solo_ramping {
                let (solo_l_buf, solo_r_buf) = self.solo.mix_buffer.split_at_mut(max_block);
                let solo_l = &mut solo_l_buf[..block_size];
                let solo_r = &mut solo_r_buf[..block_size];
                render_stereo_solo_audition(
                    StereoSoloIo {
                        out_l: solo_l,
                        out_r: solo_r,
                        dry_l: dry_left,
                        dry_r: dry_right,
                        scratch_l,
                        scratch_r,
                    },
                    active_solo_mask,
                    &self.coeff_cache.solo_coeffs,
                    &self.eq_params,
                    &mut self.solo_delay_line,
                );
                apply_equal_power_crossfade(left, solo_l, solo_start, solo_target, step);
                apply_equal_power_crossfade(right, solo_r, solo_start, solo_target, step);
            } else {
                render_stereo_solo_audition(
                    StereoSoloIo {
                        out_l: left,
                        out_r: right,
                        dry_l: dry_left,
                        dry_r: dry_right,
                        scratch_l,
                        scratch_r,
                    },
                    active_solo_mask,
                    &self.coeff_cache.solo_coeffs,
                    &self.eq_params,
                    &mut self.solo_delay_line,
                );
            }
            return;
        }

        for ch_idx in 0..channels {
            let buffer_offset = ch_idx * max_block;
            let samples = &mut self.buffer[buffer_offset..buffer_offset + block_size];
            let dry_samples = &self.bypass.dry_buffer[buffer_offset..buffer_offset + block_size];
            let scratch = &mut self.solo_scratch[buffer_offset..buffer_offset + block_size];

            if solo_ramping {
                let solo_out = &mut self.solo.mix_buffer[buffer_offset..buffer_offset + block_size];
                render_mono_solo_audition(
                    MonoSoloIo {
                        out: solo_out,
                        dry: dry_samples,
                        scratch,
                        ch_idx,
                        channels,
                    },
                    active_solo_mask,
                    &self.coeff_cache.solo_coeffs,
                    &mut self.solo_delay_line,
                );
                apply_equal_power_crossfade(samples, solo_out, solo_start, solo_target, step);
            } else {
                render_mono_solo_audition(
                    MonoSoloIo {
                        out: samples,
                        dry: dry_samples,
                        scratch,
                        ch_idx,
                        channels,
                    },
                    active_solo_mask,
                    &self.coeff_cache.solo_coeffs,
                    &mut self.solo_delay_line,
                );
            }
        }
    }

    /// Apply output gain and bypass crossfade across all active channels.
    fn apply_output_gain_and_bypass(
        &mut self,
        block_size: usize,
        output_gain_linear: f32,
        start_mix: f32,
        target_mix: f32,
        step: f32,
        is_ramping: bool,
    ) {
        let max_block = self.topology.max_block_size;
        for ch in 0..self.topology.channels {
            let offset = ch * max_block;
            let samples = &mut self.buffer[offset..offset + block_size];
            let dry_samples = &self.bypass.dry_buffer[offset..offset + block_size];

            if output_gain_linear != 1.0 {
                samples.iter_mut().for_each(|s| *s *= output_gain_linear);
            }

            if is_ramping {
                apply_equal_power_crossfade(samples, dry_samples, start_mix, target_mix, step);
            } else if self.bypass.target_bypass {
                samples.copy_from_slice(dry_samples);
            }
        }
    }

    /// Crossfade outgoing-mode scratch into `self.buffer` and clean up once complete.
    fn apply_mode_crossfade_and_advance(&mut self, block_size: usize, sample_rate: f32, step: f32) {
        if !self.mode_transition.is_ramping() {
            return;
        }

        let max_block = self.topology.max_block_size;
        let mode_start_mix = self.mode_transition.mix;
        for ch in 0..self.topology.channels {
            let offset = ch * max_block;
            let active = &mut self.buffer[offset..offset + block_size];
            let outgoing = &self.mode_transition.scratch_buffer[offset..offset + block_size];
            apply_equal_power_crossfade(active, outgoing, mode_start_mix, 0.0, step);
        }

        if self.mode_transition.advance(step, block_size) {
            if self.mode_transition.previous_mode == ProcessingMode::MinimumPhase
                && self.mode_transition.active_mode.is_linear_phase()
            {
                self.delay_line.fill(0.0);
            } else if self.mode_transition.previous_mode.is_linear_phase()
                && self.mode_transition.active_mode == ProcessingMode::MinimumPhase
            {
                self.linear_phase.update_kernel(
                    &self.eq_params,
                    sample_rate,
                    ProcessingMode::MinimumPhase,
                );
                self.dry_delay_line.reset();
            }
        }
    }

    /// Apply DSP processing to the internal flat buffer in-place.
    ///
    /// # Allocation
    ///
    /// **No allocation.** Uses pre-allocated delay-line and UPOLS convolver state.
    fn process_dsp(
        &mut self,
        block_size: usize,
        automation_provider: &Option<AutomationValueProvider>,
    ) {
        let sample_rate = self.topology.sample_rate as f32;
        let is_first_block = self.blocks_processed == 0;
        self.update_bypass_state();

        let crossfade_samples = (CROSSFADE_MS as f32 / 1000.0 * sample_rate) as usize;
        let step = 1.0_f32 / (crossfade_samples.max(1) as f32);

        let coeff_was_dirty = self.coeff_cache.dirty;
        if coeff_was_dirty {
            self.recompute_all_coefficients();
        }

        let mut block_coeffs = self.coeff_cache.coeffs;
        let any_automated = if let Some(provider) = automation_provider {
            self.apply_block_automation(block_size, sample_rate, provider, &mut block_coeffs)
        } else {
            false
        };

        let mut solo_mask: u8 = 0;
        for band_idx in 0..MAX_BANDS {
            if self.eq_params.is_band_solo(band_idx) {
                solo_mask |= 1 << band_idx;
            }
        }
        let solo_now = solo_mask != 0;
        self.solo.update(solo_now, solo_mask, is_first_block);

        let solo_target = if self.solo.target_solo {
            1.0_f32
        } else {
            0.0_f32
        };
        let solo_start = self.solo.mix;
        let solo_ramping = (solo_start - solo_target).abs() > f32::EPSILON;
        let run_solo = self.solo.target_solo || solo_ramping;
        let run_normal = !self.solo.target_solo || solo_ramping;
        let active_solo_mask = self.solo.active_mask;

        let auto_comp_db = if self.eq_params.global.auto_gain {
            self.coeff_cache.auto_gain_db * (1.0 - solo_start)
        } else {
            0.0
        };
        let output_gain_db = self.eq_params.global.output_gain_db + auto_comp_db;
        let output_gain_linear = 10.0_f32.powf(output_gain_db / 20.0);

        let target_mix = if self.bypass.target_bypass {
            1.0_f32
        } else {
            0.0_f32
        };
        let start_mix = self.bypass.mix;
        let is_ramping = (start_mix - target_mix).abs() > f32::EPSILON;

        let target_mode = self.eq_params.global.processing_mode;
        let force_tail_fade =
            self.bypass.target_bypass || is_ramping || run_solo || self.topology.channels > 2;
        let mode_changed =
            self.mode_transition
                .update(target_mode, is_first_block, force_tail_fade);

        self.sync_linear_phase_for_block(
            target_mode,
            sample_rate,
            is_first_block,
            mode_changed,
            coeff_was_dirty,
            any_automated,
        );

        if self.mode_transition.is_ramping() {
            self.render_outgoing_mode(block_size, &block_coeffs, output_gain_linear);
        }

        self.populate_dry_buffer(block_size, target_mode);

        if run_normal {
            if target_mode.is_linear_phase() {
                self.mode_transition.apply_input_fade_in(
                    &mut self.buffer,
                    self.topology.max_block_size,
                    self.topology.channels,
                    block_size,
                    step,
                );
                process_linear_phase_planar(
                    &mut self.linear_phase,
                    &mut self.buffer,
                    &self.bypass.dry_buffer,
                    self.topology.max_block_size,
                    self.topology.channels,
                    block_size,
                );
            } else {
                process_minimum_phase_planar(
                    &mut self.buffer,
                    self.topology.max_block_size,
                    self.topology.channels,
                    block_size,
                    &block_coeffs,
                    &self.eq_params,
                    &mut self.delay_line,
                );
            }
        }

        if run_solo {
            self.process_active_solo(
                block_size,
                active_solo_mask,
                solo_ramping,
                solo_start,
                solo_target,
                step,
            );
        }

        self.apply_output_gain_and_bypass(
            block_size,
            output_gain_linear,
            start_mix,
            target_mix,
            step,
            is_ramping,
        );

        self.apply_mode_crossfade_and_advance(block_size, sample_rate, step);

        self.mode_transition.record_last_rendered(
            &self.buffer,
            self.topology.max_block_size,
            self.topology.channels,
            block_size,
        );

        if solo_ramping {
            let delta = step * block_size as f32;
            self.solo.mix = if solo_start < solo_target {
                (solo_start + delta).min(solo_target)
            } else {
                (solo_start - delta).max(solo_target)
            };
        }
        if !self.solo.target_solo && self.solo.mix <= f32::EPSILON {
            self.solo.mix = 0.0;
            self.solo.active_mask = 0;
            self.solo_delay_line.fill(0.0);
        }

        if is_ramping {
            let delta = step * block_size as f32;
            self.bypass.mix = if start_mix < target_mix {
                (start_mix + delta).min(target_mix)
            } else {
                (start_mix - delta).max(target_mix)
            };
        }
    }
}

/// Process planar audio in place using the UPOLS linear-phase convolver.
fn process_linear_phase_planar(
    convolver: &mut LinearPhaseConvolver,
    buffer: &mut [f32],
    dry_buffer: &[f32],
    max_block: usize,
    channels: usize,
    block_size: usize,
) {
    if channels == 1 {
        convolver.process_mono(&mut buffer[..block_size]);
        return;
    }

    let (left_buf, right_buf) = buffer.split_at_mut(max_block);
    convolver.process_stereo(&mut left_buf[..block_size], &mut right_buf[..block_size]);

    for ch in 2..channels {
        let offset = ch * max_block;
        buffer[offset..offset + block_size]
            .copy_from_slice(&dry_buffer[offset..offset + block_size]);
    }
}

/// Process planar audio in place using the minimum-phase biquad cascade.
fn process_minimum_phase_planar(
    buffer: &mut [f32],
    max_block: usize,
    channels: usize,
    block_size: usize,
    block_coeffs: &[BiquadCoefficients; MAX_BANDS],
    eq_params: &EqParameters,
    delay_line: &mut [f32],
) {
    if channels == 2 {
        let (left_buf, right_buf) = buffer.split_at_mut(max_block);
        let left = &mut left_buf[..block_size];
        let right = &mut right_buf[..block_size];

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
            let band_delay_offset = band_idx * 2 * DELAY_SAMPLES_PER_STAGE;
            let (delay_l_slice, delay_r_slice) = delay_line
                [band_delay_offset..band_delay_offset + 2 * DELAY_SAMPLES_PER_STAGE]
                .split_at_mut(DELAY_SAMPLES_PER_STAGE);
            if let (Ok(delay_l), Ok(delay_r)) = (
                <&mut [f32; 4]>::try_from(delay_l_slice),
                <&mut [f32; 4]>::try_from(delay_r_slice),
            ) {
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
            for (band_idx, coeffs) in block_coeffs.iter().enumerate() {
                if !eq_params.bands[band_idx].enabled {
                    continue;
                }
                let routing = eq_params.band_routing(band_idx);
                if !matches!(routing, BandRouting::Mid | BandRouting::Side) {
                    continue;
                }
                let band_delay_offset = band_idx * 2 * DELAY_SAMPLES_PER_STAGE;
                let (delay_m_slice, delay_s_slice) = delay_line
                    [band_delay_offset..band_delay_offset + 2 * DELAY_SAMPLES_PER_STAGE]
                    .split_at_mut(DELAY_SAMPLES_PER_STAGE);
                if let (Ok(delay_m), Ok(delay_s)) = (
                    <&mut [f32; 4]>::try_from(delay_m_slice),
                    <&mut [f32; 4]>::try_from(delay_s_slice),
                ) {
                    match routing {
                        BandRouting::Mid => {
                            process_mono_in_place(left, coeffs, delay_m);
                        }
                        BandRouting::Side => {
                            process_mono_in_place(right, coeffs, delay_s);
                        }
                        _ => {}
                    }
                }
            }
            ms_to_lr_in_place(left, right);
        }
        return;
    }

    for ch_idx in 0..channels {
        let buffer_offset = ch_idx * max_block;
        let samples = &mut buffer[buffer_offset..buffer_offset + block_size];
        for (band_idx, coeffs) in block_coeffs.iter().enumerate() {
            if !eq_params.bands[band_idx].enabled {
                continue;
            }
            let delay_offset =
                band_idx * channels * DELAY_SAMPLES_PER_STAGE + ch_idx * DELAY_SAMPLES_PER_STAGE;
            let delay_slice = &mut delay_line[delay_offset..delay_offset + DELAY_SAMPLES_PER_STAGE];
            if let Ok(delay) = <&mut [f32; 4]>::try_from(delay_slice) {
                process_mono_in_place(samples, coeffs, delay);
            }
        }
    }
}

/// Borrowed planar slices for stereo Band Solo audition rendering.
struct StereoSoloIo<'a> {
    out_l: &'a mut [f32],
    out_r: &'a mut [f32],
    dry_l: &'a [f32],
    dry_r: &'a [f32],
    scratch_l: &'a mut [f32],
    scratch_r: &'a mut [f32],
}

/// Borrowed slices and channel metadata for mono/multi-channel Band Solo rendering.
struct MonoSoloIo<'a> {
    out: &'a mut [f32],
    dry: &'a [f32],
    scratch: &'a mut [f32],
    ch_idx: usize,
    channels: usize,
}

/// Render parallel stereo Band Solo audition into `io.out_l` / `io.out_r` from dry input.
fn render_stereo_solo_audition(
    io: StereoSoloIo<'_>,
    active_mask: u8,
    solo_coeffs: &[BiquadCoefficients; MAX_BANDS],
    eq_params: &EqParameters,
    solo_delay_line: &mut [f32],
) {
    const INV_SQRT_2: f32 = std::f32::consts::FRAC_1_SQRT_2;

    io.out_l.fill(0.0);
    io.out_r.fill(0.0);

    // Pass 1: accumulate Mid/Side solo bands in M/S domain, then convert to L/R once.
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
        let band_delay_offset = band_idx * 2 * DELAY_SAMPLES_PER_STAGE;
        let (delay_l_slice, delay_r_slice) = solo_delay_line
            [band_delay_offset..band_delay_offset + 2 * DELAY_SAMPLES_PER_STAGE]
            .split_at_mut(DELAY_SAMPLES_PER_STAGE);
        if let (Ok(delay_l), Ok(delay_r)) = (
            <&mut [f32; 4]>::try_from(delay_l_slice),
            <&mut [f32; 4]>::try_from(delay_r_slice),
        ) {
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
                    for (dst, &src) in io.out_l.iter_mut().zip(io.scratch_l.iter()) {
                        *dst += src;
                    }
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
                    for (dst, &src) in io.out_r.iter_mut().zip(io.scratch_r.iter()) {
                        *dst += src;
                    }
                }
                _ => {}
            }
        }
    }
    if has_ms_solo {
        ms_to_lr_in_place(io.out_l, io.out_r);
    }

    // Pass 2: accumulate Stereo / Left / Right solo bands directly in L/R domain.
    for (band_idx, coeffs) in solo_coeffs.iter().enumerate() {
        if (active_mask & (1 << band_idx)) == 0 {
            continue;
        }
        let routing = eq_params.band_routing(band_idx);
        if matches!(routing, BandRouting::Mid | BandRouting::Side) {
            continue;
        }
        let band_delay_offset = band_idx * 2 * DELAY_SAMPLES_PER_STAGE;
        let (delay_l_slice, delay_r_slice) = solo_delay_line
            [band_delay_offset..band_delay_offset + 2 * DELAY_SAMPLES_PER_STAGE]
            .split_at_mut(DELAY_SAMPLES_PER_STAGE);
        if let (Ok(delay_l), Ok(delay_r)) = (
            <&mut [f32; 4]>::try_from(delay_l_slice),
            <&mut [f32; 4]>::try_from(delay_r_slice),
        ) {
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
                    for (dst, &src) in io.out_l.iter_mut().zip(io.scratch_l.iter()) {
                        *dst += src;
                    }
                    for (dst, &src) in io.out_r.iter_mut().zip(io.scratch_r.iter()) {
                        *dst += src;
                    }
                }
                BandRouting::Left => {
                    io.scratch_l.copy_from_slice(io.dry_l);
                    process_mono_in_place(io.scratch_l, coeffs, delay_l);
                    for (dst, &src) in io.out_l.iter_mut().zip(io.scratch_l.iter()) {
                        *dst += src;
                    }
                }
                BandRouting::Right => {
                    io.scratch_r.copy_from_slice(io.dry_r);
                    process_mono_in_place(io.scratch_r, coeffs, delay_r);
                    for (dst, &src) in io.out_r.iter_mut().zip(io.scratch_r.iter()) {
                        *dst += src;
                    }
                }
                BandRouting::Mid | BandRouting::Side => {}
            }
        }
    }
}

/// Render parallel mono/multi-channel Band Solo audition into `io.out` from `io.dry`.
fn render_mono_solo_audition(
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
        for (dst, &src) in io.out.iter_mut().zip(io.scratch.iter()) {
            *dst += src;
        }
    }
}

/// Apply continuous equal-power sin/cos crossfade between `samples` (`mix == 0.0`) and
/// `other_samples` (`mix == 1.0`).
#[inline]
fn apply_equal_power_crossfade(
    samples: &mut [f32],
    other_samples: &[f32],
    start_mix: f32,
    target_mix: f32,
    step: f32,
) {
    let mut mix = start_mix;
    for (sample, &other) in samples.iter_mut().zip(other_samples.iter()) {
        let angle = mix * std::f32::consts::FRAC_PI_2;
        let primary_gain = angle.cos();
        let secondary_gain = angle.sin();
        *sample = *sample * primary_gain + other * secondary_gain;

        if mix < target_mix {
            mix = (mix + step).min(target_mix);
        } else if mix > target_mix {
            mix = (mix - step).max(target_mix);
        }
    }
}
