//! Standalone `GuiClient` implementation bridging `egui` with `equaform-engine`.

use equaform_analysis::AnalysisEngine;
use equaform_common::BandParameters;
use equaform_common::state::Preset;
use equaform_engine::EngineLifecycle;

use crate::preset_manager::PresetManager;
use crate::{
    AnalysisSnapshot, GuiClient, GuiCommand, GuiCommandBatch, ParamId, ParamSnapshot, ParamValue,
    default_8_bands,
};

/// A real engine client that wraps the actual equaform-engine.
pub(super) struct EngineClient {
    pub(super) engine: Option<Box<EngineLifecycle>>,
    analysis_engine: AnalysisEngine,
    pub(super) snapshot: ParamSnapshot,
    pending_commands: GuiCommandBatch,
    pub(super) preset_manager: PresetManager,
    egui_context: egui::Context,
    /// Band parameters for the frequency response plot.
    pub(super) band_params: [equaform_common::BandParameters; equaform_common::MAX_BANDS],
    /// Sample rate for the frequency response plot.
    pub(super) sample_rate: f32,
    /// Currently selected preset name for the combo box.
    pub(super) _current_preset: String,
    /// Spectrum processor for real-time FFT visualization.
    _spectrum_processor: equaform_analysis::SpectrumProcessor,
    /// Running sample counter for spectrum frame numbering.
    _spectrum_frame_number: u64,
}

impl EngineClient {
    pub(super) fn new(egui_context: egui::Context) -> Self {
        Self {
            engine: None,
            analysis_engine: AnalysisEngine::new(),
            snapshot: ParamSnapshot::new(Vec::new()),
            pending_commands: GuiCommandBatch::new(),
            preset_manager: PresetManager::new(),
            egui_context,
            band_params: core::array::from_fn(|i| equaform_common::BandParameters {
                frequency_hz: 20.0 * (i + 1) as f32,
                ..Default::default()
            }),
            sample_rate: 48000.0,
            _current_preset: String::new(),
            _spectrum_processor: equaform_analysis::SpectrumProcessor::new(
                equaform_analysis::SpectrumConfig::new(1024, 48000.0),
            ),
            _spectrum_frame_number: 0,
        }
    }

    pub(super) fn take_pending_commands(&mut self) -> GuiCommandBatch {
        std::mem::take(&mut self.pending_commands)
    }

    /// Update band parameters from a command batch.
    #[allow(dead_code)]
    fn apply_params_from_batch(&mut self, batch: &GuiCommandBatch) {
        let commands = batch.commands();
        for cmd in commands {
            let pid = cmd.param_id.0;
            // Global parameters (1..=99)
            if (1..=99).contains(&pid) {
                continue;
            }
            if pid >= 100 {
                let band_idx = ((pid - 100) / 100) as usize;
                let offset = ((pid - 100) % 100) as usize;
                if band_idx < equaform_common::MAX_BANDS {
                    let bp = &mut self.band_params[band_idx];
                    match offset {
                        0 => bp.enabled = cmd.value > 0.5,
                        1 => bp.filter_type = Self::filter_type_from_f32(cmd.value),
                        2 => bp.frequency_hz = cmd.value,
                        3 => bp.gain_db = cmd.value.clamp(-24.0, 24.0),
                        4 => bp.q = cmd.value.clamp(0.1, 20.0),
                        5 => bp.slope = Self::slope_from_f32(cmd.value),
                        _ => {}
                    }
                }
            }
        }
    }

    /// Map a float value to a FilterType enum variant.
    fn filter_type_from_f32(v: f32) -> equaform_common::FilterType {
        equaform_common::FilterType::from_raw(v.round().max(0.0) as u32)
            .unwrap_or(equaform_common::FilterType::Bell)
    }

    /// Map a float value to a Slope enum variant.
    fn slope_from_f32(v: f32) -> equaform_common::Slope {
        equaform_common::Slope::from_raw(v.round().max(0.0) as u32)
            .unwrap_or(equaform_common::Slope::Db12)
    }

    /// Map a FilterType enum to a float value (for snapshot encoding).
    fn filter_type_to_f32(ft: equaform_common::FilterType) -> f32 {
        ft.to_raw() as f32
    }

    /// Map a Slope enum to a float value (for snapshot encoding).
    fn slope_to_f32(sl: equaform_common::Slope) -> f32 {
        sl.to_raw() as f32
    }

    /// Map a ProcessingMode enum to a float value (for snapshot encoding).
    fn processing_mode_to_f32(pm: equaform_common::ProcessingMode) -> f32 {
        pm.to_raw() as f32
    }

    /// Map a StereoMode enum to a float value (for snapshot encoding).
    fn stereo_mode_to_f32(_sm: equaform_common::StereoMode) -> f32 {
        0.0
    }

    /// Encode the current EqParameters into a ParamSnapshot for widget sync.
    pub(super) fn encode_snapshot(&self) -> Vec<ParamValue> {
        let mut values: Vec<ParamValue> = Vec::new();

        if let Some(ref eng) = self.engine {
            let params = eng.eq_parameters();

            // Global parameters
            values.push(ParamValue {
                param_id: ParamId(1),
                value: if params.global.bypass { 1.0 } else { 0.0 },
            });
            values.push(ParamValue {
                param_id: ParamId(2),
                value: params.global.output_gain_db,
            });
            values.push(ParamValue {
                param_id: ParamId(3),
                value: if params.global.auto_gain { 1.0 } else { 0.0 },
            });
            values.push(ParamValue {
                param_id: ParamId(4),
                value: Self::processing_mode_to_f32(params.global.processing_mode),
            });
            values.push(ParamValue {
                param_id: ParamId(5),
                value: Self::stereo_mode_to_f32(params.global.stereo_mode),
            });
            values.push(ParamValue {
                param_id: ParamId(equaform_common::global::AUTO_GAIN_COMPENSATION_DB),
                value: eng.auto_gain_compensation_db(),
            });
            values.push(ParamValue {
                param_id: ParamId(equaform_common::global::AUTO_GAIN_MODE),
                value: params.global.auto_gain_mode.to_raw() as f32,
            });

            // Band parameters
            for band_idx in 0..equaform_common::MAX_BANDS {
                let base = 100u32 + band_idx as u32 * 100;
                let band = &params.bands[band_idx];
                values.push(ParamValue {
                    param_id: ParamId(base),
                    value: if band.enabled { 1.0 } else { 0.0 },
                });
                values.push(ParamValue {
                    param_id: ParamId(base + 1),
                    value: Self::filter_type_to_f32(band.filter_type),
                });
                values.push(ParamValue {
                    param_id: ParamId(base + 2),
                    value: band.frequency_hz,
                });
                values.push(ParamValue {
                    param_id: ParamId(base + 3),
                    value: band.gain_db,
                });
                values.push(ParamValue {
                    param_id: ParamId(base + 4),
                    value: band.q,
                });
                values.push(ParamValue {
                    param_id: ParamId(base + 5),
                    value: Self::slope_to_f32(band.slope),
                });
                values.push(ParamValue {
                    param_id: ParamId(base + equaform_common::band::SOLO),
                    value: if params.is_band_solo(band_idx) {
                        1.0
                    } else {
                        0.0
                    },
                });
                values.push(ParamValue {
                    param_id: ParamId(base + equaform_common::band::ROUTING),
                    value: params.band_routing(band_idx).to_raw() as f32,
                });
            }
        }

        values
    }

    /// Apply a single GuiCommand to EqParameters.
    pub(super) fn apply_command_to_params(
        params: &mut equaform_common::EqParameters,
        cmd: &GuiCommand,
    ) {
        let pid = cmd.param_id.0;
        if (1..=99).contains(&pid) {
            // Global parameters
            match pid {
                1 => params.global.bypass = cmd.value > 0.5,
                2 => params.global.output_gain_db = cmd.value.clamp(-24.0, 24.0),
                3 => params.global.auto_gain = cmd.value > 0.5,
                equaform_common::global::PROCESSING_MODE => {
                    if cmd.value.is_finite() {
                        let raw = cmd.value.round().clamp(0.0, 2.0) as u32;
                        if let Some(mode) = equaform_common::ProcessingMode::from_raw(raw) {
                            params.global.processing_mode = mode;
                        }
                    }
                }
                5 => {
                    params.global.stereo_mode = equaform_common::StereoMode::Stereo;
                }
                7 => {
                    let raw = cmd.value.round().clamp(0.0, 1.0) as u32;
                    if let Some(mode) = equaform_common::AutoGainMode::from_raw(raw) {
                        params.global.auto_gain_mode = mode;
                    }
                }
                _ => {}
            }
        } else if pid >= 100 {
            // Band parameters: band_idx = (pid - 100) / 100
            let band_idx = ((pid - 100) / 100) as usize;
            if band_idx >= equaform_common::MAX_BANDS {
                return;
            }
            let offset = ((pid - 100) % 100) as usize;
            let band = &mut params.bands[band_idx];
            match offset {
                0 => band.enabled = cmd.value > 0.5,
                1 => band.filter_type = Self::filter_type_from_f32(cmd.value),
                2 => band.frequency_hz = cmd.value,
                3 => band.gain_db = cmd.value.clamp(-24.0, 24.0),
                4 => band.q = cmd.value.clamp(0.1, 20.0),
                5 => band.slope = Self::slope_from_f32(cmd.value),
                6 => params.set_band_solo(band_idx, cmd.value > 0.5),
                7 => {
                    let raw = cmd.value.round().clamp(0.0, 4.0) as u32;
                    if let Some(routing) = equaform_common::BandRouting::from_raw(raw) {
                        params.set_band_routing(band_idx, routing);
                    }
                }
                _ => {}
            }
        }
    }

    /// Drain analysis frames from the engine into the analysis engine.
    fn drain_analysis_frames(&mut self) {
        if let Some(ref mut eng) = self.engine {
            let abuf = eng.analysis_buffer();
            while let Some(frame) = abuf.pop() {
                self.analysis_engine.push(frame);
            }
        }
    }
}

impl GuiClient for EngineClient {
    fn prepare(&mut self, sample_rate: f32, block_size: usize, channels: usize) {
        // Destroy existing engine if present.
        if let Some(ref mut eng) = self.engine {
            eng.destroy();
        }

        // Create a new engine safely on the heap to avoid stack overflow.
        let Some(mut eng) = std::thread::Builder::new()
            .name("engine-client-init".to_string())
            .stack_size(8 * 1024 * 1024)
            .spawn(|| Box::new(EngineLifecycle::new()))
            .ok()
            .and_then(|handle| handle.join().ok())
        else {
            self.engine = None;
            self.sample_rate = sample_rate;
            return;
        };

        if eng.prepare(sample_rate as f64, block_size, channels) {
            // Initialize band_params to default 8-band musical layout.
            self.band_params = default_8_bands();
            let mut params = *eng.eq_parameters();
            params.bands = self.band_params;
            eng.set_parameters(params);
            self.engine = Some(eng);

            // Update snapshot from engine params.
            self.snapshot = ParamSnapshot::new(self.encode_snapshot());
        } else {
            self.engine = None;
        }

        self.sample_rate = sample_rate;
    }

    fn process(&mut self, input: &[f32], output: &mut [f32]) {
        let eng = match self.engine.as_mut() {
            Some(e) => e,
            None => return,
        };

        // Apply pending commands first.
        if !self.pending_commands.is_empty() {
            let batch = std::mem::take(&mut self.pending_commands);
            let mut params = *eng.eq_parameters();
            for cmd in batch.commands() {
                Self::apply_command_to_params(&mut params, cmd);
            }
            eng.set_parameters(params);
        }

        // Convert flat interleaved → planar for engine.
        let channels = 2; // stereo default
        let block_size = input.len() / channels;
        let input_planar: [&[f32]; 2] = [&input[0..block_size], &input[block_size..]];
        let (lo, hi) = output.split_at_mut(block_size);
        let mut output_planar: [&mut [f32]; 2] = [lo, hi];

        if !eng.process(&input_planar, &mut output_planar) {
            // Dry copy fallback on processing failure
            output_planar[0].copy_from_slice(input_planar[0]);
            output_planar[1].copy_from_slice(input_planar[1]);
        }

        // Update snapshot and band_params from engine before releasing borrow.
        let eq_params = *eng.eq_parameters();

        // Drain analysis frames.
        self.drain_analysis_frames();

        // Update snapshot from engine params.
        self.snapshot = ParamSnapshot::new(self.encode_snapshot());

        // Update band_params from engine.
        for i in 0..equaform_common::MAX_BANDS {
            self.band_params[i] = eq_params.bands[i];
        }
    }

    fn snapshot(&self) -> ParamSnapshot {
        self.snapshot.clone()
    }

    fn analysis_snapshot(&self) -> AnalysisSnapshot {
        let mut rms = [f32::NEG_INFINITY; equaform_common::ANALYSIS_MAX_CHANNELS];
        let mut peak = [f32::NEG_INFINITY; equaform_common::ANALYSIS_MAX_CHANNELS];
        let mut dropped = false;

        // Pull the latest analysis frame if available.
        if let Some(frame) = self.analysis_engine.latest_snapshot() {
            for (i, &v) in frame.rms.iter().enumerate() {
                if i < equaform_common::ANALYSIS_MAX_CHANNELS {
                    rms[i] = v;
                    peak[i] = frame.peak[i];
                }
            }
            dropped = frame.dropped;
        }

        AnalysisSnapshot {
            rms,
            peak,
            dropped,
            spectrum_data: Vec::new(),
        }
    }

    fn apply_commands(&mut self, batch: GuiCommandBatch) {
        if let Some(ref mut eng) = self.engine {
            let mut params = *eng.eq_parameters();
            for cmd in batch.commands() {
                Self::apply_command_to_params(&mut params, cmd);
            }
            eng.set_parameters(params);
            // Sync band_params from engine so the UI always reflects engine state.
            let eq_params = *eng.eq_parameters();
            for i in 0..equaform_common::MAX_BANDS {
                self.band_params[i] = eq_params.bands[i];
            }
            // Update snapshot from engine parameters.
            self.snapshot = ParamSnapshot::new(self.encode_snapshot());
        } else {
            for cmd in batch.commands() {
                self.pending_commands.push(cmd.param_id, cmd.value);
            }
        }
    }

    fn destroy(&mut self) {
        if let Some(ref mut eng) = self.engine {
            eng.destroy();
        }
        self.engine = None;
    }

    fn is_prepared(&self) -> bool {
        self.engine.is_some()
    }

    fn create_preset(&mut self, name: &str) -> Result<String, String> {
        let _params = self
            .engine
            .as_ref()
            .map(|e| *e.eq_parameters())
            .unwrap_or_default();

        let mut preset = Preset::new(name);
        for pv in self.encode_snapshot().into_iter() {
            // Skip read-only telemetry and monitor-only parameters not in the persisted contract.
            if equaform_common::parameter::parameter_info(pv.param_id.0).is_none() {
                continue;
            }
            preset.parameter_values.set(pv.param_id.0, pv.value);
        }

        // Allow updating an existing preset with the same name.
        self.preset_manager.remove(name);
        self.preset_manager
            .add(name.to_string(), preset)
            .map_err(|e| format!("create preset: {e}"))?;
        Ok(name.to_string())
    }

    fn apply_preset(&mut self, name: &str) -> Result<(), String> {
        let preset = self
            .preset_manager
            .get(name)
            .ok_or_else(|| format!("preset '{name}' not found"))?;

        if let Some(ref mut eng) = self.engine {
            let mut params = *eng.eq_parameters();
            let keep_bypass = params.global.bypass;
            let keep_auto_gain = params.global.auto_gain;
            let keep_auto_gain_mode = params.global.auto_gain_mode;
            for (id, value) in preset.preset.parameter_values.iter() {
                let pid = ParamId(id);
                Self::apply_command_to_params(
                    &mut params,
                    &GuiCommand {
                        param_id: pid,
                        value,
                    },
                );
            }
            // Preserve global Bypass and Auto Gain state when switching presets.
            params.global.bypass = keep_bypass;
            params.global.auto_gain = keep_auto_gain;
            params.global.auto_gain_mode = keep_auto_gain_mode;
            eng.set_parameters(params);
        }

        // Update snapshot and band_params locally.
        self.snapshot = ParamSnapshot::new(self.encode_snapshot());
        let eq_params = self
            .engine
            .as_ref()
            .map(|e| *e.eq_parameters())
            .unwrap_or_default();
        for i in 0..equaform_common::MAX_BANDS {
            self.band_params[i] = eq_params.bands[i];
        }

        Ok(())
    }

    fn list_presets(&self) -> Vec<String> {
        crate::presets_io::sorted_preset_names(self.preset_manager.iter().map(|p| p.name.as_str()))
    }

    fn remove_preset(&mut self, name: &str) -> bool {
        self.preset_manager.remove(name)
    }

    fn save_preset_to_file(&mut self, name: &str, path: &str) -> Result<(), String> {
        self.preset_manager
            .save_to_file(name, path)
            .map_err(|e| format!("save preset: {e}"))
    }

    fn load_preset_from_file(&mut self, path: &str) -> Result<String, String> {
        let named = self
            .preset_manager
            .load_from_file(path)
            .map_err(|e| format!("load preset: {e}"))?;

        if let Some(ref mut eng) = self.engine {
            let mut params = *eng.eq_parameters();
            let keep_bypass = params.global.bypass;
            let keep_auto_gain = params.global.auto_gain;
            let keep_auto_gain_mode = params.global.auto_gain_mode;
            for (id, value) in named.preset.parameter_values.iter() {
                let pid = ParamId(id);
                Self::apply_command_to_params(
                    &mut params,
                    &GuiCommand {
                        param_id: pid,
                        value,
                    },
                );
            }
            params.global.bypass = keep_bypass;
            params.global.auto_gain = keep_auto_gain;
            params.global.auto_gain_mode = keep_auto_gain_mode;
            eng.set_parameters(params);
        }

        // Update snapshot and band_params locally.
        self.snapshot = ParamSnapshot::new(self.encode_snapshot());
        let eq_params = self
            .engine
            .as_ref()
            .map(|e| *e.eq_parameters())
            .unwrap_or_default();
        for i in 0..equaform_common::MAX_BANDS {
            self.band_params[i] = eq_params.bands[i];
        }

        Ok(named.name)
    }

    fn egui_context(&self) -> &egui::Context {
        &self.egui_context
    }

    fn next_auto_name(&self) -> String {
        self.preset_manager.next_auto_name()
    }

    fn band_parameters(&self, band_index: usize) -> Option<BandParameters> {
        if band_index < equaform_common::MAX_BANDS {
            Some(self.band_params[band_index])
        } else {
            None
        }
    }

    fn spectrum_data(&self) -> &[[f64; 2]] {
        // Return an empty slice; `analysis_snapshot()` provides spectrum data for the UI plot.
        static EMPTY: &[[f64; 2]] = &[];
        EMPTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use equaform_common::{ProcessingMode, global};

    #[test]
    fn test_engine_client_processing_mode_roundtrip() {
        let mut client = EngineClient::new(egui::Context::default());
        client.prepare(48_000.0, 256, 2);

        assert_eq!(
            client.snapshot().get(ParamId(global::PROCESSING_MODE)),
            Some(0.0),
            "initial PROCESSING_MODE should default to MinimumPhase (0.0)"
        );

        let cases = [
            (ProcessingMode::MinimumPhase, 0.0),
            (ProcessingMode::LinearPhaseLow, 1.0),
            (ProcessingMode::LinearPhaseHigh, 2.0),
            (ProcessingMode::MinimumPhase, 0.0),
        ];

        for (mode, raw) in cases {
            let mut batch = GuiCommandBatch::new();
            batch.push(ParamId(global::PROCESSING_MODE), raw);
            client.apply_commands(batch);

            assert_eq!(
                client.snapshot().get(ParamId(global::PROCESSING_MODE)),
                Some(raw),
                "snapshot PROCESSING_MODE must round-trip {raw}"
            );
            assert_eq!(
                client
                    .engine
                    .as_ref()
                    .map(|e| e.eq_parameters().global.processing_mode),
                Some(mode),
                "engine processing_mode must match {mode:?}"
            );
        }

        // Non-finite input must not panic or corrupt current mode.
        let mut set_high = GuiCommandBatch::new();
        set_high.push(ParamId(global::PROCESSING_MODE), 2.0);
        client.apply_commands(set_high);

        let mut nan_batch = GuiCommandBatch::new();
        nan_batch.push(ParamId(global::PROCESSING_MODE), f32::NAN);
        client.apply_commands(nan_batch);

        assert_eq!(
            client
                .engine
                .as_ref()
                .map(|e| e.eq_parameters().global.processing_mode),
            Some(ProcessingMode::LinearPhaseHigh)
        );
    }

    #[test]
    fn test_engine_client_preset_processing_mode_and_legacy_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut client = EngineClient::new(egui::Context::default());
        client.prepare(48_000.0, 256, 2);

        let mut batch = GuiCommandBatch::new();
        batch.push(ParamId(global::PROCESSING_MODE), 2.0);
        client.apply_commands(batch);
        assert_eq!(
            client.snapshot().get(ParamId(global::PROCESSING_MODE)),
            Some(2.0)
        );

        client
            .create_preset("Linear Master")
            .map_err(std::io::Error::other)?;

        let mut reset_batch = GuiCommandBatch::new();
        reset_batch.push(ParamId(global::PROCESSING_MODE), 0.0);
        client.apply_commands(reset_batch);
        assert_eq!(
            client.snapshot().get(ParamId(global::PROCESSING_MODE)),
            Some(0.0)
        );

        client
            .apply_preset("Linear Master")
            .map_err(std::io::Error::other)?;
        assert_eq!(
            client.snapshot().get(ParamId(global::PROCESSING_MODE)),
            Some(2.0),
            "preset with PROCESSING_MODE must restore LinearPhaseHigh (2.0)"
        );

        // Legacy preset without PROCESSING_MODE preserves active user mode.
        let mut legacy = Preset::new("Legacy NoMode");
        legacy.parameter_values.set(103, 3.0);
        client
            .preset_manager
            .add("Legacy NoMode".to_string(), legacy)?;
        client
            .apply_preset("Legacy NoMode")
            .map_err(std::io::Error::other)?;
        assert_eq!(
            client.snapshot().get(ParamId(global::PROCESSING_MODE)),
            Some(2.0)
        );

        Ok(())
    }
}
