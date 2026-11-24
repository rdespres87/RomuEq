//! Real-time audio processing, automation ingestion, biquad cascade, and click-free bypass crossfade.

use super::{CROSSFADE_MS, DELAY_SAMPLES_PER_STAGE, EngineLifecycle, LifecycleState};
use equaform_analysis::AnalysisFrame;
use equaform_common::automation::AutomationValueProvider;
use equaform_common::{BandParameters, MAX_BANDS, ParamId};
use equaform_dsp::coefficients::{BiquadCoefficients, FilterSpecification};

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
            panic!("inputs.len() != outputs.len()");
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

        let auto_comp_db = if self.eq_params.global.auto_gain {
            self.coeff_cache.auto_gain_db
        } else {
            0.0
        };
        let output_gain_db = self.eq_params.global.output_gain_db + auto_comp_db;
        let output_gain_linear = 10.0_f32.powf(output_gain_db / 20.0);

        let is_entering_bypass = self.bypass.curr_bypass;
        let start_crossfade_progress = self.bypass.crossfade_progress;

        for ch_idx in 0..self.topology.channels {
            let buffer_offset = ch_idx * self.topology.max_block_size;
            let samples = &mut self.buffer[buffer_offset..buffer_offset + block_size];
            let dry_samples = &mut self.bypass.dry_buffer[buffer_offset..buffer_offset + block_size];

            dry_samples.copy_from_slice(samples);

            for (band_idx, coeffs) in block_coeffs.iter().enumerate() {
                if !self.eq_params.bands[band_idx].enabled {
                    continue;
                }
                let delay_offset = band_idx * self.topology.channels * DELAY_SAMPLES_PER_STAGE
                    + ch_idx * DELAY_SAMPLES_PER_STAGE;
                let delay =
                    &mut self.delay_line[delay_offset..delay_offset + DELAY_SAMPLES_PER_STAGE];
                apply_biquad_stage(samples, coeffs, delay);
            }

            if output_gain_linear != 1.0 {
                samples.iter_mut().for_each(|s| *s *= output_gain_linear);
            }

            if self.bypass.transitioning && start_crossfade_progress < crossfade_samples {
                apply_bypass_crossfade(
                    samples,
                    dry_samples,
                    start_crossfade_progress,
                    crossfade_samples,
                    is_entering_bypass,
                );
            } else if self.bypass.curr_bypass {
                samples.copy_from_slice(dry_samples);
            }
        }

        if self.bypass.transitioning {
            self.bypass.crossfade_progress += block_size;
            if self.bypass.crossfade_progress >= crossfade_samples {
                self.bypass.transitioning = false;
            }
        }
    }
}

/// Apply a single direct-form I biquad stage in-place using pre-allocated delay state.
#[inline]
fn apply_biquad_stage(samples: &mut [f32], coeffs: &BiquadCoefficients, delay: &mut [f32]) {
    let b0 = coeffs.b0;
    let b1 = coeffs.b1;
    let b2 = coeffs.b2;
    let a1 = coeffs.a1;
    let a2 = coeffs.a2;

    let mut x1 = delay[0];
    let mut x2 = delay[1];
    let mut y1 = delay[2];
    let mut y2 = delay[3];

    for sample in samples.iter_mut() {
        let x_n = *sample;
        let y_n = b0 * x_n + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        *sample = y_n;
        x2 = x1;
        x1 = x_n;
        y2 = y1;
        y1 = y_n;
    }

    delay[0] = x1;
    delay[1] = x2;
    delay[2] = y1;
    delay[3] = y2;
}

/// Apply equal-power sin/cos crossfade between wet (`samples`) and `dry_samples` in-place.
#[inline]
fn apply_bypass_crossfade(
    samples: &mut [f32],
    dry_samples: &[f32],
    start_progress: usize,
    crossfade_samples: usize,
    is_entering_bypass: bool,
) {
    for (i, (sample, &dry)) in samples.iter_mut().zip(dry_samples.iter()).enumerate() {
        let progress = (start_progress + i).min(crossfade_samples);
        let t = progress as f32 / crossfade_samples as f32;
        let sin_t = (t * std::f32::consts::FRAC_PI_2).sin();
        let cos_t = (t * std::f32::consts::FRAC_PI_2).cos();

        let (wet_gain, dry_gain) = if is_entering_bypass {
            (cos_t, sin_t)
        } else {
            (sin_t, cos_t)
        };

        *sample = *sample * wet_gain + dry * dry_gain;
    }
}
