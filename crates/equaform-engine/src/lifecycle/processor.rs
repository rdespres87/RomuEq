//! Real-time audio processing, automation ingestion, DSP routing, and click-free transitions.

use super::render::{
    BlockLayout, MonoSoloIo, RampParams, StereoSoloIo, apply_equal_power_crossfade,
    process_linear_phase_planar, process_minimum_phase_planar, render_mono_solo_audition,
    render_stereo_solo_audition,
};
use super::{CROSSFADE_MS, EngineLifecycle, LifecycleState};
use equaform_analysis::AnalysisFrame;
use equaform_common::automation::AutomationValueProvider;
use equaform_common::{BandParameters, MAX_BANDS, ParamId, ProcessingMode};
use equaform_dsp::coefficients::{BiquadCoefficients, FilterSpecification};

/// Per-block flags controlling linear-phase kernel synchronization.
#[derive(Clone, Copy, Debug)]
struct KernelSyncFlags {
    is_first_block: bool,
    mode_changed: bool,
    coeff_was_dirty: bool,
    any_automated: bool,
}

/// Prepared per-block DSP control state computed before rendering.
#[derive(Clone, Copy, Debug)]
struct PreparedBlockState {
    step: f32,
    target_mode: ProcessingMode,
    sync_flags: KernelSyncFlags,
    active_solo_mask: u8,
    run_solo: bool,
    run_normal: bool,
    solo_ramp: RampParams,
    bypass_ramp: RampParams,
    output_gain_linear: f32,
}

impl EngineLifecycle {
    /// Create an automation value provider for the current block.
    fn create_automation_provider(&mut self, block_size: usize) -> Option<AutomationValueProvider> {
        let ingestor = self.automation_ingestor.as_mut()?;
        let batch = ingestor.process_block(block_size as u32);
        Some(ingestor.value_provider(&batch))
    }

    /// Return current planar block layout for `block_size` samples.
    #[inline]
    fn block_layout(&self, block_size: usize) -> BlockLayout {
        BlockLayout {
            max_block: self.topology.max_block_size,
            channels: self.topology.channels,
            block_size,
        }
    }

    /// Process a single audio block.
    ///
    /// # Allocation
    ///
    /// **No allocation.** All data is copied to/from the flat pre-allocated buffer.
    /// DSP processing uses pre-allocated delay-line and UPOLS convolver state.
    pub fn process(&mut self, inputs: &[&[f32]], outputs: &mut [&mut [f32]]) -> bool {
        if self.state != LifecycleState::Ready || inputs.len() != outputs.len() {
            return false;
        }

        let block_size = inputs.first().map(|s| s.len()).unwrap_or(0);
        if block_size == 0 || block_size > self.topology.max_block_size {
            return false;
        }

        if inputs.iter().any(|s| s.len() != block_size)
            || outputs.iter().any(|s| s.len() != block_size)
            || inputs.len() != self.topology.channels
        {
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

    /// Return cached linear output gain for `output_gain_db`, recomputing only on bit changes.
    #[inline]
    fn resolve_output_gain_linear(&mut self, output_gain_db: f32) -> f32 {
        if output_gain_db.to_bits() != self.coeff_cache.last_output_gain_db.to_bits() {
            self.coeff_cache.last_output_gain_db = output_gain_db;
            self.coeff_cache.output_gain_linear = 10.0_f32.powf(output_gain_db / 20.0);
        }
        self.coeff_cache.output_gain_linear
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
        let gain_range = BandParameters::gain_db_max() - gain_min;
        let freq_min: f32 = 20.0;
        let freq_ratio: f32 = (sample_rate / 2.0) / freq_min;

        let mut any_automated = false;
        for (band_idx, coeff_slot) in block_coeffs.iter_mut().enumerate() {
            let mut band_params = self.eq_params.bands[band_idx];
            if !band_params.enabled {
                continue;
            }

            let mut automated = false;
            if let Some(automation_value) =
                last_or_sample(provider, band_params.gain_param_id(), block_length)
            {
                band_params.gain_db = gain_min + (automation_value + 1.0) * 0.5 * gain_range;
                automated = true;
            }

            if let Some(automation_value) =
                last_or_sample(provider, band_params.frequency_param_id(), block_length)
            {
                let t = automation_value * 0.5 + 0.5;
                band_params.frequency_hz = freq_min * freq_ratio.powf(t);
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

    /// Compute active solo mask, update solo transition state, and return `(active_mask, solo_ramp)`.
    fn prepare_solo_state(&mut self, is_first_block: bool, step: f32) -> (u8, RampParams) {
        let solo_mask = (0..MAX_BANDS).fold(0_u8, |mask, band_idx| {
            if self.eq_params.is_band_solo(band_idx) {
                mask | (1 << band_idx)
            } else {
                mask
            }
        });
        self.solo.update(solo_mask != 0, solo_mask, is_first_block);

        let target = if self.solo.target_solo { 1.0 } else { 0.0 };
        let start = self.solo.mix;
        let ramp = RampParams {
            start,
            target,
            step,
            ramping: (start - target).abs() > f32::EPSILON,
        };
        (self.solo.active_mask, ramp)
    }

    /// Prepare coefficients, solo/bypass/mode ramps, and cached output gain for this block.
    fn prepare_block_state(
        &mut self,
        block_size: usize,
        sample_rate: f32,
        automation_provider: &Option<AutomationValueProvider>,
        block_coeffs: &mut [BiquadCoefficients; MAX_BANDS],
    ) -> PreparedBlockState {
        let is_first_block = self.blocks_processed == 0;
        self.update_bypass_state();

        let crossfade_samples = (CROSSFADE_MS as f32 / 1000.0 * sample_rate) as usize;
        let step = 1.0_f32 / (crossfade_samples.max(1) as f32);

        let coeff_was_dirty = self.coeff_cache.dirty;
        if coeff_was_dirty {
            self.recompute_all_coefficients();
        }

        *block_coeffs = self.coeff_cache.coeffs;
        let any_automated = match automation_provider {
            Some(provider) => {
                self.apply_block_automation(block_size, sample_rate, provider, block_coeffs)
            }
            None => false,
        };

        let (active_solo_mask, solo_ramp) = self.prepare_solo_state(is_first_block, step);
        let run_solo = self.solo.target_solo || solo_ramp.ramping;
        let run_normal = !self.solo.target_solo || solo_ramp.ramping;

        let auto_comp_db = if self.eq_params.global.auto_gain {
            self.coeff_cache.auto_gain_db * (1.0 - solo_ramp.start)
        } else {
            0.0
        };
        let output_gain_linear =
            self.resolve_output_gain_linear(self.eq_params.global.output_gain_db + auto_comp_db);

        let target_mix = if self.bypass.target_bypass { 1.0 } else { 0.0 };
        let start_mix = self.bypass.mix;
        let is_ramping = (start_mix - target_mix).abs() > f32::EPSILON;
        let bypass_ramp = RampParams {
            start: start_mix,
            target: target_mix,
            step,
            ramping: is_ramping,
        };

        let target_mode = self.eq_params.global.processing_mode;
        let force_tail_fade =
            self.bypass.target_bypass || is_ramping || run_solo || self.topology.channels > 2;
        let mode_changed =
            self.mode_transition
                .update(target_mode, is_first_block, force_tail_fade);

        PreparedBlockState {
            step,
            target_mode,
            sync_flags: KernelSyncFlags {
                is_first_block,
                mode_changed,
                coeff_was_dirty,
                any_automated,
            },
            active_solo_mask,
            run_solo,
            run_normal,
            solo_ramp,
            bypass_ramp,
            output_gain_linear,
        }
    }

    /// Synchronize the linear-phase convolver and dry delay line for mode/parameter updates.
    fn sync_linear_phase_for_block(
        &mut self,
        target_mode: ProcessingMode,
        sample_rate: f32,
        flags: KernelSyncFlags,
    ) {
        if flags.is_first_block {
            if target_mode.is_linear_phase() {
                self.linear_phase
                    .update_kernel(&self.eq_params, sample_rate, target_mode);
                self.linear_phase.reset();
                self.dry_delay_line.reset();
            }
            return;
        }

        if flags.mode_changed {
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

        if (flags.coeff_was_dirty || flags.any_automated) && target_mode.is_linear_phase() {
            self.linear_phase
                .request_kernel_update(&self.eq_params, sample_rate, target_mode);
        }
    }

    /// Render the outgoing mode (or smooth tail-fade reference) into `mode_transition.scratch_buffer`.
    fn render_outgoing_mode(
        &mut self,
        layout: BlockLayout,
        block_coeffs: &[BiquadCoefficients; MAX_BANDS],
        output_gain_linear: f32,
    ) {
        if self.mode_transition.use_tail_fade {
            for (ch, &tail) in self.mode_transition.prev_output[..layout.channels]
                .iter()
                .enumerate()
            {
                let offset = ch * layout.max_block;
                self.mode_transition.scratch_buffer[offset..offset + layout.block_size].fill(tail);
            }
            return;
        }

        for ch in 0..layout.channels {
            let offset = ch * layout.max_block;
            self.mode_transition.scratch_buffer[offset..offset + layout.block_size]
                .copy_from_slice(&self.buffer[offset..offset + layout.block_size]);
        }

        if self.mode_transition.previous_mode == ProcessingMode::MinimumPhase {
            process_minimum_phase_planar(
                &mut self.mode_transition.scratch_buffer,
                layout,
                block_coeffs,
                &self.eq_params,
                &mut self.delay_line,
            );
        } else {
            process_linear_phase_planar(
                &mut self.linear_phase,
                &mut self.mode_transition.scratch_buffer,
                &self.bypass.dry_buffer,
                layout,
            );
        }

        if output_gain_linear != 1.0 {
            for ch in 0..layout.channels {
                let offset = ch * layout.max_block;
                self.mode_transition.scratch_buffer[offset..offset + layout.block_size]
                    .iter_mut()
                    .for_each(|s| *s *= output_gain_linear);
            }
        }
    }

    /// Populate `bypass.dry_buffer` with either zero-latency or latency-aligned dry audio.
    fn populate_dry_buffer(&mut self, layout: BlockLayout, target_mode: ProcessingMode) {
        if target_mode.is_linear_phase() {
            let delay = target_mode.latency_samples() as usize;
            for ch in 0..layout.channels {
                let offset = ch * layout.max_block;
                let input = &self.buffer[offset..offset + layout.block_size];
                let dry = &mut self.bypass.dry_buffer[offset..offset + layout.block_size];
                self.dry_delay_line.process_channel(ch, input, dry, delay);
            }
        } else {
            for ch in 0..layout.channels {
                let offset = ch * layout.max_block;
                self.bypass.dry_buffer[offset..offset + layout.block_size]
                    .copy_from_slice(&self.buffer[offset..offset + layout.block_size]);
            }
        }
    }

    /// Apply Band Solo audition into `self.buffer` from the latency-aligned `bypass.dry_buffer`.
    fn process_active_solo(&mut self, layout: BlockLayout, active_solo_mask: u8, ramp: RampParams) {
        if layout.channels == 2 {
            self.process_stereo_solo(layout, active_solo_mask, ramp);
        } else {
            self.process_mono_solo(layout, active_solo_mask, ramp);
        }
    }

    /// Apply stereo Band Solo audition and optional crossfade into `self.buffer`.
    fn process_stereo_solo(&mut self, layout: BlockLayout, active_solo_mask: u8, ramp: RampParams) {
        let (left_buf, right_buf) = self.buffer.split_at_mut(layout.max_block);
        let left = &mut left_buf[..layout.block_size];
        let right = &mut right_buf[..layout.block_size];

        let (dry_left_buf, dry_right_buf) = self.bypass.dry_buffer.split_at_mut(layout.max_block);
        let dry_l = &dry_left_buf[..layout.block_size];
        let dry_r = &dry_right_buf[..layout.block_size];

        let (scratch_l_buf, scratch_r_buf) = self.solo_scratch.split_at_mut(layout.max_block);
        let scratch_l = &mut scratch_l_buf[..layout.block_size];
        let scratch_r = &mut scratch_r_buf[..layout.block_size];

        if ramp.ramping {
            let (solo_l_buf, solo_r_buf) = self.solo.mix_buffer.split_at_mut(layout.max_block);
            let solo_l = &mut solo_l_buf[..layout.block_size];
            let solo_r = &mut solo_r_buf[..layout.block_size];
            render_stereo_solo_audition(
                StereoSoloIo {
                    out_l: solo_l,
                    out_r: solo_r,
                    dry_l,
                    dry_r,
                    scratch_l,
                    scratch_r,
                },
                active_solo_mask,
                &self.coeff_cache.solo_coeffs,
                &self.eq_params,
                &mut self.solo_delay_line,
            );
            apply_equal_power_crossfade(left, solo_l, ramp);
            apply_equal_power_crossfade(right, solo_r, ramp);
        } else {
            render_stereo_solo_audition(
                StereoSoloIo {
                    out_l: left,
                    out_r: right,
                    dry_l,
                    dry_r,
                    scratch_l,
                    scratch_r,
                },
                active_solo_mask,
                &self.coeff_cache.solo_coeffs,
                &self.eq_params,
                &mut self.solo_delay_line,
            );
        }
    }

    /// Apply mono/multi-channel Band Solo audition and optional crossfade into `self.buffer`.
    fn process_mono_solo(&mut self, layout: BlockLayout, active_solo_mask: u8, ramp: RampParams) {
        for ch_idx in 0..layout.channels {
            let offset = ch_idx * layout.max_block;
            let samples = &mut self.buffer[offset..offset + layout.block_size];
            let dry = &self.bypass.dry_buffer[offset..offset + layout.block_size];
            let scratch = &mut self.solo_scratch[offset..offset + layout.block_size];

            if ramp.ramping {
                let solo_out = &mut self.solo.mix_buffer[offset..offset + layout.block_size];
                render_mono_solo_audition(
                    MonoSoloIo {
                        out: solo_out,
                        dry,
                        scratch,
                        ch_idx,
                        channels: layout.channels,
                    },
                    active_solo_mask,
                    &self.coeff_cache.solo_coeffs,
                    &mut self.solo_delay_line,
                );
                apply_equal_power_crossfade(samples, solo_out, ramp);
            } else {
                render_mono_solo_audition(
                    MonoSoloIo {
                        out: samples,
                        dry,
                        scratch,
                        ch_idx,
                        channels: layout.channels,
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
        layout: BlockLayout,
        output_gain_linear: f32,
        ramp: RampParams,
    ) {
        for ch in 0..layout.channels {
            let offset = ch * layout.max_block;
            let samples = &mut self.buffer[offset..offset + layout.block_size];
            let dry_samples = &self.bypass.dry_buffer[offset..offset + layout.block_size];

            if output_gain_linear != 1.0 {
                samples.iter_mut().for_each(|s| *s *= output_gain_linear);
            }

            if ramp.ramping {
                apply_equal_power_crossfade(samples, dry_samples, ramp);
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
        let mode_ramp = RampParams {
            start: self.mode_transition.mix,
            target: 0.0,
            step,
            ramping: true,
        };
        for ch in 0..self.topology.channels {
            let offset = ch * max_block;
            let active = &mut self.buffer[offset..offset + block_size];
            let outgoing = &self.mode_transition.scratch_buffer[offset..offset + block_size];
            apply_equal_power_crossfade(active, outgoing, mode_ramp);
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

    /// Render normal EQ, solo audition, output gain, bypass, and mode crossfades for a block.
    fn render_block_dsp(
        &mut self,
        layout: BlockLayout,
        state: &PreparedBlockState,
        block_coeffs: &[BiquadCoefficients; MAX_BANDS],
        sample_rate: f32,
    ) {
        self.populate_dry_buffer(layout, state.target_mode);

        if state.run_normal {
            if state.target_mode.is_linear_phase() {
                self.mode_transition
                    .apply_input_fade_in(&mut self.buffer, layout, state.step);
                process_linear_phase_planar(
                    &mut self.linear_phase,
                    &mut self.buffer,
                    &self.bypass.dry_buffer,
                    layout,
                );
            } else {
                process_minimum_phase_planar(
                    &mut self.buffer,
                    layout,
                    block_coeffs,
                    &self.eq_params,
                    &mut self.delay_line,
                );
            }
        }

        if state.run_solo {
            self.process_active_solo(layout, state.active_solo_mask, state.solo_ramp);
        }

        self.apply_output_gain_and_bypass(layout, state.output_gain_linear, state.bypass_ramp);
        self.apply_mode_crossfade_and_advance(layout.block_size, sample_rate, state.step);
        self.mode_transition
            .record_last_rendered(&self.buffer, layout);
    }

    /// Advance solo and bypass ramp state after rendering a block.
    fn advance_ramp_states(
        &mut self,
        block_size: usize,
        solo_ramp: RampParams,
        bypass_ramp: RampParams,
    ) {
        if solo_ramp.ramping {
            self.solo.mix = solo_ramp.next_mix(block_size);
        }
        if !self.solo.target_solo && self.solo.mix <= f32::EPSILON {
            self.solo.mix = 0.0;
            self.solo.active_mask = 0;
            self.solo_delay_line.fill(0.0);
        }

        if bypass_ramp.ramping {
            self.bypass.mix = bypass_ramp.next_mix(block_size);
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
        let layout = self.block_layout(block_size);
        let mut block_coeffs = [BiquadCoefficients::default(); MAX_BANDS];
        let state = self.prepare_block_state(
            block_size,
            sample_rate,
            automation_provider,
            &mut block_coeffs,
        );

        self.sync_linear_phase_for_block(state.target_mode, sample_rate, state.sync_flags);

        if self.mode_transition.is_ramping() {
            self.render_outgoing_mode(layout, &block_coeffs, state.output_gain_linear);
        }

        self.render_block_dsp(layout, &state, &block_coeffs, sample_rate);
        self.advance_ramp_states(block_size, state.solo_ramp, state.bypass_ramp);
    }
}

/// Read the latest automation value for `param_id` in the block or at its final sample.
#[inline]
fn last_or_sample(
    provider: &AutomationValueProvider,
    param_id: ParamId,
    block_length: u32,
) -> Option<f32> {
    provider
        .last_value_in_block(param_id, block_length)
        .or_else(|| provider.value_at_sample(param_id, block_length.saturating_sub(1), None))
}
