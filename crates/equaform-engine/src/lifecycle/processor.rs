//! Real-time audio processing, automation ingestion, biquad cascade, and click-free bypass crossfade.

use super::{CROSSFADE_MS, DELAY_SAMPLES_PER_STAGE, EngineLifecycle, LifecycleState};
use equaform_analysis::AnalysisFrame;
use equaform_common::automation::AutomationValueProvider;
use equaform_common::{BandParameters, BandRouting, MAX_BANDS, ParamId};
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

    /// Query the automation provider for a parameter value at a given sample.
    #[allow(dead_code)]
    fn query_automation(
        &self,
        provider: &Option<AutomationValueProvider>,
        param_id: ParamId,
        sample_in_block: usize,
    ) -> Option<f32> {
        provider.as_ref().and_then(|p| {
            let sample = self.sample_offset as u32 + sample_in_block as u32;
            p.value_at_sample(param_id, sample, None)
        })
    }

    /// Process a single audio block.
    ///
    /// # Allocation
    ///
    /// **No allocation.** All data is copied to/from the flat pre-allocated buffer.
    /// DSP processing uses pre-allocated biquad delay-line state.
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

        let frame = AnalysisFrame::compute(
            &self.buffer[..block_size * self.topology.channels],
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
    fn apply_block_automation(
        &mut self,
        block_size: usize,
        sample_rate: f32,
        provider: &AutomationValueProvider,
        block_coeffs: &mut [BiquadCoefficients; MAX_BANDS],
    ) {
        let block_length = block_size as u32;
        let gain_min = BandParameters::gain_db_min();
        let gain_max = BandParameters::gain_db_max();
        let freq_min: f32 = 20.0;
        let freq_max: f32 = sample_rate / 2.0;

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
                if self.eq_params.global.auto_gain {
                    self.update_auto_gain_db();
                }
            }
        }
    }

    /// Apply DSP processing to the internal flat buffer in-place.
    ///
    /// # Allocation
    ///
    /// **No allocation.** Uses pre-allocated delay-line state.
    fn process_dsp(
        &mut self,
        block_size: usize,
        automation_provider: &Option<AutomationValueProvider>,
    ) {
        let sample_rate = self.topology.sample_rate as f32;
        self.update_bypass_state();

        let crossfade_samples = (CROSSFADE_MS as f32 / 1000.0 * sample_rate) as usize;

        if self.coeff_cache.dirty {
            self.recompute_all_coefficients();
        }

        let mut block_coeffs = self.coeff_cache.coeffs;
        if let Some(provider) = automation_provider {
            self.apply_block_automation(block_size, sample_rate, provider, &mut block_coeffs);
        }

        let solo_active = self.eq_params.any_band_solo();
        if !solo_active && self.solo_was_active {
            self.solo_delay_line.fill(0.0);
        }
        self.solo_was_active = solo_active;

        let auto_comp_db = if self.eq_params.global.auto_gain && !solo_active {
            self.coeff_cache.auto_gain_db
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
        let step = 1.0_f32 / (crossfade_samples.max(1) as f32);
        let is_ramping = (start_mix - target_mix).abs() > f32::EPSILON;

        if self.topology.channels == 2 {
            let max_block = self.topology.max_block_size;
            let (left_buf, right_buf) = self.buffer.split_at_mut(max_block);
            let left = &mut left_buf[..block_size];
            let right = &mut right_buf[..block_size];

            let (dry_left_buf, dry_right_buf) = self.bypass.dry_buffer.split_at_mut(max_block);
            let dry_left = &mut dry_left_buf[..block_size];
            let dry_right = &mut dry_right_buf[..block_size];

            dry_left.copy_from_slice(left);
            dry_right.copy_from_slice(right);

            if solo_active {
                left.fill(0.0);
                right.fill(0.0);
                let (scratch_l_buf, scratch_r_buf) = self.solo_scratch.split_at_mut(max_block);
                let scratch_l = &mut scratch_l_buf[..block_size];
                let scratch_r = &mut scratch_r_buf[..block_size];

                for (band_idx, solo_coeffs) in self.coeff_cache.solo_coeffs.iter().enumerate() {
                    if !self.eq_params.is_band_solo(band_idx) {
                        continue;
                    }
                    let routing = self.eq_params.band_routing(band_idx);
                    scratch_l.copy_from_slice(dry_left);
                    scratch_r.copy_from_slice(dry_right);
                    let band_delay_offset = band_idx * 2 * DELAY_SAMPLES_PER_STAGE;
                    let (delay_l_slice, delay_r_slice) = self.solo_delay_line
                        [band_delay_offset..band_delay_offset + 2 * DELAY_SAMPLES_PER_STAGE]
                        .split_at_mut(DELAY_SAMPLES_PER_STAGE);
                    if let (Ok(delay_l), Ok(delay_r)) = (
                        <&mut [f32; 4]>::try_from(delay_l_slice),
                        <&mut [f32; 4]>::try_from(delay_r_slice),
                    ) {
                        match routing {
                            BandRouting::Stereo => {
                                process_stereo_planar_in_place(
                                    scratch_l,
                                    scratch_r,
                                    solo_coeffs,
                                    delay_l,
                                    delay_r,
                                );
                            }
                            BandRouting::Left => {
                                process_mono_in_place(scratch_l, solo_coeffs, delay_l);
                                scratch_r.fill(0.0);
                            }
                            BandRouting::Right => {
                                scratch_l.fill(0.0);
                                process_mono_in_place(scratch_r, solo_coeffs, delay_r);
                            }
                            BandRouting::Mid => {
                                lr_to_ms_in_place(scratch_l, scratch_r);
                                process_mono_in_place(scratch_l, solo_coeffs, delay_l);
                                scratch_r.fill(0.0);
                                ms_to_lr_in_place(scratch_l, scratch_r);
                            }
                            BandRouting::Side => {
                                lr_to_ms_in_place(scratch_l, scratch_r);
                                scratch_l.fill(0.0);
                                process_mono_in_place(scratch_r, solo_coeffs, delay_r);
                                ms_to_lr_in_place(scratch_l, scratch_r);
                            }
                        }
                    }
                    for (dst, &src) in left.iter_mut().zip(scratch_l.iter()) {
                        *dst += src;
                    }
                    for (dst, &src) in right.iter_mut().zip(scratch_r.iter()) {
                        *dst += src;
                    }
                }
            } else {
                let mut has_ms_band = false;
                for (band_idx, coeffs) in block_coeffs.iter().enumerate() {
                    if !self.eq_params.bands[band_idx].enabled {
                        continue;
                    }
                    let routing = self.eq_params.band_routing(band_idx);
                    if matches!(routing, BandRouting::Mid | BandRouting::Side) {
                        has_ms_band = true;
                        continue;
                    }
                    let band_delay_offset = band_idx * 2 * DELAY_SAMPLES_PER_STAGE;
                    let (delay_l_slice, delay_r_slice) = self.delay_line
                        [band_delay_offset..band_delay_offset + 2 * DELAY_SAMPLES_PER_STAGE]
                        .split_at_mut(DELAY_SAMPLES_PER_STAGE);
                    if let (Ok(delay_l), Ok(delay_r)) = (
                        <&mut [f32; 4]>::try_from(delay_l_slice),
                        <&mut [f32; 4]>::try_from(delay_r_slice),
                    ) {
                        match routing {
                            BandRouting::Stereo => {
                                process_stereo_planar_in_place(
                                    left, right, coeffs, delay_l, delay_r,
                                );
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
                        if !self.eq_params.bands[band_idx].enabled {
                            continue;
                        }
                        let routing = self.eq_params.band_routing(band_idx);
                        if !matches!(routing, BandRouting::Mid | BandRouting::Side) {
                            continue;
                        }
                        let band_delay_offset = band_idx * 2 * DELAY_SAMPLES_PER_STAGE;
                        let (delay_m_slice, delay_s_slice) = self.delay_line
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
            }

            if output_gain_linear != 1.0 {
                for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                    *l *= output_gain_linear;
                    *r *= output_gain_linear;
                }
            }

            if is_ramping {
                apply_bypass_crossfade(left, dry_left, start_mix, target_mix, step);
                apply_bypass_crossfade(right, dry_right, start_mix, target_mix, step);
            } else if self.bypass.target_bypass {
                left.copy_from_slice(dry_left);
                right.copy_from_slice(dry_right);
            }
        } else {
            for ch_idx in 0..self.topology.channels {
                let buffer_offset = ch_idx * self.topology.max_block_size;
                let samples = &mut self.buffer[buffer_offset..buffer_offset + block_size];
                let dry_samples =
                    &mut self.bypass.dry_buffer[buffer_offset..buffer_offset + block_size];

                dry_samples.copy_from_slice(samples);

                if solo_active {
                    samples.fill(0.0);
                    let scratch = &mut self.solo_scratch[buffer_offset..buffer_offset + block_size];
                    for (band_idx, solo_coeffs) in self.coeff_cache.solo_coeffs.iter().enumerate() {
                        if !self.eq_params.is_band_solo(band_idx) {
                            continue;
                        }
                        scratch.copy_from_slice(dry_samples);
                        let delay_offset =
                            band_idx * self.topology.channels * DELAY_SAMPLES_PER_STAGE
                                + ch_idx * DELAY_SAMPLES_PER_STAGE;
                        let delay_slice = &mut self.solo_delay_line
                            [delay_offset..delay_offset + DELAY_SAMPLES_PER_STAGE];
                        if let Ok(delay) = <&mut [f32; 4]>::try_from(delay_slice) {
                            process_mono_in_place(scratch, solo_coeffs, delay);
                        }
                        for (dst, &src) in samples.iter_mut().zip(scratch.iter()) {
                            *dst += src;
                        }
                    }
                } else {
                    for (band_idx, coeffs) in block_coeffs.iter().enumerate() {
                        if !self.eq_params.bands[band_idx].enabled {
                            continue;
                        }
                        let delay_offset =
                            band_idx * self.topology.channels * DELAY_SAMPLES_PER_STAGE
                                + ch_idx * DELAY_SAMPLES_PER_STAGE;
                        let delay_slice = &mut self.delay_line
                            [delay_offset..delay_offset + DELAY_SAMPLES_PER_STAGE];
                        if let Ok(delay) = <&mut [f32; 4]>::try_from(delay_slice) {
                            process_mono_in_place(samples, coeffs, delay);
                        }
                    }
                }

                if output_gain_linear != 1.0 {
                    samples.iter_mut().for_each(|s| *s *= output_gain_linear);
                }

                if is_ramping {
                    apply_bypass_crossfade(samples, dry_samples, start_mix, target_mix, step);
                } else if self.bypass.target_bypass {
                    samples.copy_from_slice(dry_samples);
                }
            }
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

/// Apply continuous equal-power sin/cos crossfade between wet (`samples`) and `dry_samples`.
///
/// `start_mix` and `target_mix` are in `[0.0, 1.0]` where `0.0` is fully wet and `1.0` is
/// fully dry (bypassed). Reversing direction mid-fade continues smoothly from `start_mix`.
#[inline]
fn apply_bypass_crossfade(
    samples: &mut [f32],
    dry_samples: &[f32],
    start_mix: f32,
    target_mix: f32,
    step: f32,
) {
    let mut mix = start_mix;
    for (sample, &dry) in samples.iter_mut().zip(dry_samples.iter()) {
        let angle = mix * std::f32::consts::FRAC_PI_2;
        let wet_gain = angle.cos();
        let dry_gain = angle.sin();
        *sample = *sample * wet_gain + dry * dry_gain;

        if mix < target_mix {
            mix = (mix + step).min(target_mix);
        } else if mix > target_mix {
            mix = (mix - step).max(target_mix);
        }
    }
}
