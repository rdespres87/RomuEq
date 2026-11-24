//! Standalone desktop runner for Equaform (eframe/winit + CoreAudio/CPAL).

use crate::*;
use eframe::App;

use equaform_common::state::Preset;

/// Run the GUI application as a standalone desktop app.
pub fn run(app: impl GuiApp) -> eframe::Result {
    let (w, h) = app.window_size();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([w, h])
            .with_min_inner_size([960.0, 475.0])
            .with_resizable(true),
        ..Default::default()
    };
    let window_title = app.window_title().to_string();
    eframe::run_native(
        &window_title,
        options,
        Box::new(move |cc| {
            let ppp = cc.egui_ctx.pixels_per_point();
            cc.egui_ctx.set_pixels_per_point(ppp);
            Ok(Box::new(StandaloneRunner::new(app, cc.egui_ctx.clone())))
        }),
    )
}

/// A runner that bridges the GuiApp trait with eframe::App.
struct StandaloneRunner<A: GuiApp> {
    app: A,
    client: EngineClient,
    audio_stream: Option<audio::AudioStream>,
    audio_active: bool,
    available_outputs: Vec<String>,
    available_inputs: Vec<String>,
    ui_state: WorkspaceUiState,
    spectrum_processor: equaform_analysis::SpectrumProcessor,
    spectrum_frame_num: u64,
    status_message: Option<String>,
    analysis: AnalysisSnapshot,
    sample_buffer: Vec<f32>,
    silence_frames: u32,
    was_in_zero_fps: bool,
}

impl<A: GuiApp> StandaloneRunner<A> {
    fn new(app: A, ctx: egui::Context) -> Self {
        let mut client = EngineClient::new();
        client.set_egui_context(ctx);
        // Initialize the engine at startup so encode_snapshot() returns real data.
        client.prepare(48000.0, 512, 2);

        // Populate built-in presets, load user presets from disk, and restore full saved state BEFORE starting audio
        initialize_client_presets(&mut client);

        let mut audio_stream = audio::AudioStream::new();

        // Auto-detect audio devices for seamless macOS EQ replacement
        let in_devices = audio::AudioStream::available_input_devices();
        let out_devices = audio::AudioStream::available_output_devices();

        // Detect virtual input bridge (Equaform prioritized, then legacy RomuEq)
        let detected_in = in_devices
            .iter()
            .find(|d| d.contains("Equaform"))
            .or_else(|| in_devices.iter().find(|d| d.contains("RomuEq")))
            .cloned();

        // Detect real hardware output (restore last selected device if available, else built-in / external)
        let saved_out = load_saved_output_device();
        let detected_out = saved_out
            .and_then(|s| out_devices.iter().find(|d| **d == s).cloned())
            .or_else(|| {
                out_devices
                    .iter()
                    .find(|d| {
                        let l = d.to_lowercase();
                        l.contains("haut-parleurs")
                            || l.contains("macbook")
                            || l.contains("internal")
                            || l.contains("built-in")
                    })
                    .cloned()
            })
            .or_else(|| {
                out_devices
                    .iter()
                    .find(|d| {
                        let l = d.to_lowercase();
                        !l.contains("equaform") && !l.contains("romueq")
                    })
                    .cloned()
            })
            .or_else(|| out_devices.first().cloned());

        let initial_mode = audio::AudioSourceMode::PassThrough;

        audio_stream.set_source_mode(initial_mode);
        audio_stream.set_input_device(detected_in.clone());
        audio_stream.set_output_device(detected_out.clone());
        if let Some(ref eng) = client.engine {
            let _ = audio_stream.set_parameters(*eng.eq_parameters());
        }

        let (audio_active, status_message) = match audio_stream.start() {
            Ok(()) => {
                client.sample_rate = audio_stream.sample_rate();
                let out_name = detected_out.as_deref().unwrap_or("Output");
                (true, Some(format!("EQ active → {out_name}")))
            }
            Err(e) => (false, Some(format!("Stream startup error: {e}"))),
        };

        let spectrum_config = equaform_analysis::SpectrumConfig::new(1024, 48000.0);
        let spectrum_processor = equaform_analysis::SpectrumProcessor::new(spectrum_config);
        let analysis = AnalysisSnapshot {
            spectrum_data: Vec::with_capacity(513),
            ..Default::default()
        };
        let sample_buffer = Vec::with_capacity(2048);

        Self {
            app,
            client,
            audio_stream: Some(audio_stream),
            audio_active,
            available_outputs: out_devices,
            available_inputs: in_devices,
            ui_state: WorkspaceUiState::default(),
            spectrum_processor,
            spectrum_frame_num: 0,
            status_message,
            analysis,
            sample_buffer,
            silence_frames: 0,
            was_in_zero_fps: false,
        }
    }
}

impl<A: GuiApp> App for StandaloneRunner<A> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let is_focused = ctx.input(|i| i.focused);
        let is_minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        let has_interaction =
            ctx.input(|i| i.pointer.any_down() || i.pointer.delta() != egui::Vec2::ZERO);

        if is_minimized {
            self.was_in_zero_fps = true;
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
            return;
        }

        if self.was_in_zero_fps {
            if let Some(ref audio) = self.audio_stream {
                audio.clear_spectrum_buffer();
            }
            self.was_in_zero_fps = false;
        }

        ctx.set_visuals(egui::Visuals::dark());

        // Update DPI scaling.
        let _pixels_per_point = ctx.pixels_per_point();

        // Query engine state.
        let _snapshot = self.client.snapshot();
        let engine_analysis = self.client.analysis_snapshot();
        self.analysis.dropped = engine_analysis.dropped;

        // Compute real FFT spectrum from audio stream if running
        if self.audio_active {
            if let Some(ref audio) = self.audio_stream {
                let (peak_l, peak_r) = audio.peak_levels_db();
                self.analysis.peak[0] = peak_l;
                self.analysis.peak[1] = peak_r;
                self.analysis.rms[0] = peak_l;
                self.analysis.rms[1] = peak_r;

                audio.copy_latest_output_samples(&mut self.sample_buffer);
                let max_peak = peak_l.max(peak_r);
                let is_silent = max_peak <= -90.0 || self.sample_buffer.is_empty();

                if is_silent {
                    self.silence_frames = self.silence_frames.saturating_add(1);
                } else if max_peak > -84.0 {
                    self.silence_frames = 0;
                }

                if self.silence_frames < 15 && !self.sample_buffer.is_empty() {
                    let sr = audio.sample_rate();
                    let frame = self.spectrum_processor.process_ref(
                        &self.sample_buffer,
                        sr as f64,
                        self.spectrum_frame_num,
                    );
                    self.spectrum_frame_num += 1;
                    let dft_norm_db = 20.0 * ((frame.fft_size as f64) * 0.5).log10();
                    self.analysis.spectrum_data.clear();
                    self.analysis
                        .spectrum_data
                        .extend((0..frame.num_bins).map(|b| {
                            [
                                frame.bin_frequency(b),
                                (frame.magnitudes_db[b] as f64) - dft_norm_db,
                            ]
                        }));
                } else {
                    self.analysis.spectrum_data.clear();
                }
            }
        } else {
            self.analysis.spectrum_data.clear();
        }

        // Process any pending commands from client.
        let pending = self.client.take_pending_commands();
        if !pending.is_empty() {
            self.client.apply_commands(pending);
            // Synchronize live parameters with audio thread
            if let Some(ref mut audio) = self.audio_stream {
                if let Some(ref engine) = self.client.engine {
                    let _ = audio.set_parameters(*engine.eq_parameters());
                }
            }
        }

        // Update GUI app with current state.
        self.app.update(&mut self.client);

        // Render the UI.
        // Render the unified workspace UI across Standalone and Plugin modes.
        let env = HostEnvironment::Standalone {
            audio_active: self.audio_active,
            current_output: self
                .audio_stream
                .as_ref()
                .and_then(|a| a.selected_output_device())
                .unwrap_or("Défaut Système")
                .to_string(),
            available_outputs: self.available_outputs.clone(),
            peak_levels_db: self
                .audio_stream
                .as_ref()
                .map(|a| a.peak_levels_db())
                .unwrap_or((-100.0, -100.0)),
        };

        let mut batch = GuiCommandBatch::new();
        let mut actions = Vec::new();

        egui::CentralPanel::default().show(ctx, |ui| {
            let ws_ctx = WorkspaceUiContext {
                env: &env,
                band_params: &self.client.band_params,
                sample_rate: self.client.sample_rate,
                snapshot: &self.client.snapshot,
                analysis: &self.analysis,
                preset_manager: &self.client.preset_manager,
                current_preset: &self.client._current_preset,
                status_message: self.status_message.as_deref(),
            };
            let (b, a) = render_workspace(ui, &mut self.ui_state, &ws_ctx);
            batch = b;
            actions = a;
        });

        // Handle actions from workspace UI
        for action in actions {
            match action {
                WorkspaceAction::TogglePlayStop => {
                    if let Some(ref mut audio) = self.audio_stream {
                        if self.audio_active {
                            let _ = audio.stop();
                            self.audio_active = false;
                            self.status_message = Some("Audio muted".to_string());
                        } else {
                            if let Some(ref engine) = self.client.engine {
                                let _ = audio.set_parameters(*engine.eq_parameters());
                            }
                            match audio.start() {
                                Ok(()) => {
                                    self.audio_active = true;
                                    self.client.sample_rate = audio.sample_rate();
                                    let out_name =
                                        audio.selected_output_device().unwrap_or("Output");
                                    self.status_message =
                                        Some(format!("EQ active → {out_name}"));
                                }
                                Err(err) => {
                                    self.status_message = Some(format!("Audio error: {err}"));
                                }
                            }
                        }
                    }
                }
                WorkspaceAction::SelectOutputDevice(dev) => {
                    if let Some(ref mut audio) = self.audio_stream {
                        audio.set_output_device(Some(dev.clone()));
                        save_selected_output_device(&dev);
                        if let Some(ref engine) = self.client.engine {
                            let _ = audio.set_parameters(*engine.eq_parameters());
                        }
                        if self.audio_active {
                            let _ = audio.stop();
                            match audio.start() {
                                Ok(()) => {
                                    self.audio_active = true;
                                    self.client.sample_rate = audio.sample_rate();
                                    self.status_message = Some(format!("EQ active → {dev}"));
                                }
                                Err(e) => {
                                    self.audio_active = false;
                                    self.status_message =
                                        Some(format!("Output error '{dev}': {e}"));
                                }
                            }
                        } else {
                            self.status_message = Some(format!("Selected output: {dev}"));
                        }
                    }
                }
                WorkspaceAction::RefreshDevices => {
                    self.available_outputs = audio::AudioStream::available_output_devices();
                    self.available_inputs = audio::AudioStream::available_input_devices();
                    self.status_message = Some("Audio devices refreshed".to_string());
                }
                WorkspaceAction::ApplyPreset(name) => {
                    let _ = self.client.apply_preset(&name);
                    self.client._current_preset = name.clone();
                    let mut engage = GuiCommandBatch::new();
                    engage.push(ParamId(1), 0.0);
                    self.client.apply_commands(engage);
                    save_last_active_preset_name(&name);
                    save_complete_app_state(&self.client);
                    if let Some(ref mut audio) = self.audio_stream {
                        if let Some(ref engine) = self.client.engine {
                            let _ = audio.set_parameters(*engine.eq_parameters());
                        }
                    }
                    self.status_message = Some(format!("Preset '{name}' applied"));
                }
                WorkspaceAction::SavePreset(name) => {
                    if let Err(e) = self.client.create_preset(&name) {
                        self.status_message = Some(format!("Preset error '{name}': {e}"));
                    } else {
                        self.client._current_preset = name.clone();
                        let _ = persist_preset_to_disk(&mut self.client.preset_manager, &name);
                        save_last_active_preset_name(&name);
                        save_complete_app_state(&self.client);
                        self.status_message = Some(format!("Preset '{name}' saved to disk"));
                    }
                }
                WorkspaceAction::DeletePreset(name) => {
                    let _ = self.client.remove_preset(&name);
                    remove_preset_from_disk(&name);
                    self.client._current_preset.clear();
                    save_complete_app_state(&self.client);
                    self.status_message = Some(format!("Preset '{name}' deleted"));
                }
            }
        }

        // Process the batch through the engine client.
        if !batch.is_empty() {
            let modifies_band = batch.commands().iter().any(|c| c.param_id.0 >= 100);
            let modifies_bypass = batch.commands().iter().any(|c| c.param_id.0 == 1);
            if modifies_band && !modifies_bypass {
                let cur_bypass = self
                    .client
                    .snapshot()
                    .get(ParamId(1))
                    .map(|v| v >= 0.5)
                    .unwrap_or(false);
                if cur_bypass {
                    batch.push(ParamId(1), 0.0);
                }
            }
            self.client.apply_commands(batch);
            save_complete_app_state(&self.client);
            if let Some(ref mut audio) = self.audio_stream {
                if let Some(ref engine) = self.client.engine {
                    let _ = audio.set_parameters(*engine.eq_parameters());
                }
            }
        }

        // Guarantee that the real-time audio thread always has the exact latest EqParameters
        if let Some(ref mut audio) = self.audio_stream {
            if let Some(ref engine) = self.client.engine {
                audio.ensure_parameters_synced(*engine.eq_parameters());
            }
        }

        // Repaint adaptively based on focus and user interaction.
        if is_focused || has_interaction {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(66));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        save_complete_app_state(&self.client);
        if !self.client._current_preset.is_empty() {
            save_last_active_preset_name(&self.client._current_preset);
        }
        if let Some(ref mut audio) = self.audio_stream {
            let _ = audio.stop();
            audio.restore_system_output_to_selected();
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Sleek studio dark background
        [0.10, 0.11, 0.13, 1.0]
    }
}

fn user_config_dir() -> std::path::PathBuf {
    crate::presets_io::user_config_dir()
}

fn save_selected_output_device(dev: &str) {
    let path = user_config_dir().join("last_output_device.txt");
    let _ = std::fs::write(path, dev.as_bytes());
}

fn load_saved_output_device() -> Option<String> {
    let path = user_config_dir().join("last_output_device.txt");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn save_complete_app_state(client: &EngineClient) {
    let mut state = equaform_common::state::ProjectState::new();
    for pv in client.encode_snapshot() {
        if pv.param_id.0 == 6 {
            continue;
        }
        state.set(pv.param_id.0, pv.value);
    }
    let bytes = state.serialize();
    let path = user_config_dir().join("app_state.bin");
    let _ = std::fs::write(path, bytes);
}

fn load_complete_app_state(client: &mut EngineClient) -> bool {
    let path = user_config_dir().join("app_state.bin");
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let Ok(state) = equaform_common::state::ProjectState::deserialize(&bytes) else {
        return false;
    };
    if let Some(ref mut eng) = client.engine {
        let mut params = *eng.eq_parameters();
        for (id, value) in state.parameter_values.iter() {
            if id == 6 {
                continue;
            }
            EngineClient::apply_command_to_params(
                &mut params,
                &GuiCommand {
                    param_id: ParamId(id),
                    value,
                },
            );
        }
        eng.set_parameters(params);
    }
    client.snapshot = ParamSnapshot::new(client.encode_snapshot());
    let eq_params = client
        .engine
        .as_ref()
        .map(|e| *e.eq_parameters())
        .unwrap_or_default();
    for i in 0..equaform_common::MAX_BANDS {
        client.band_params[i] = eq_params.bands[i];
    }
    true
}

fn initialize_client_presets(client: &mut EngineClient) {
    crate::presets_io::populate_factory_presets(&mut client.preset_manager);

    let initial_preset = load_last_active_preset_name()
        .filter(|n| client.preset_manager.get(n).is_some())
        .unwrap_or_else(|| "Flat".to_string());
    let _ = client.apply_preset(&initial_preset);
    client._current_preset = initial_preset;

    // Restore complete saved app state (bypass, auto_gain, master gain, and exact band curves) if available
    let _ = load_complete_app_state(client);
}

use equaform_analysis::AnalysisEngine;
use equaform_engine::EngineLifecycle;

/// A real engine client that wraps the actual equaform-engine.
struct EngineClient {
    engine: Option<Box<EngineLifecycle>>,
    analysis_engine: AnalysisEngine,
    snapshot: ParamSnapshot,
    pending_commands: GuiCommandBatch,
    preset_manager: PresetManager,
    egui_context: Option<egui::Context>,
    /// Band parameters for the frequency response plot.
    band_params: [equaform_common::BandParameters; equaform_common::MAX_BANDS],
    /// Sample rate for the frequency response plot.
    sample_rate: f32,
    /// Currently selected preset name for the combo box.
    _current_preset: String,
    /// Spectrum processor for real-time FFT visualization.
    _spectrum_processor: equaform_analysis::SpectrumProcessor,
    /// Running sample counter for spectrum frame numbering.
    _spectrum_frame_number: u64,
}

impl EngineClient {
    fn new() -> Self {
        Self {
            engine: None,
            analysis_engine: AnalysisEngine::new(),
            snapshot: ParamSnapshot::new(Vec::new()),
            pending_commands: GuiCommandBatch::new(),
            preset_manager: PresetManager::new(),
            egui_context: None,
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

    fn set_egui_context(&mut self, ctx: egui::Context) {
        self.egui_context = Some(ctx);
    }

    fn take_pending_commands(&mut self) -> GuiCommandBatch {
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
    fn processing_mode_to_f32(_pm: equaform_common::ProcessingMode) -> f32 {
        0.0
    }

    /// Map a StereoMode enum to a float value (for snapshot encoding).
    fn stereo_mode_to_f32(_sm: equaform_common::StereoMode) -> f32 {
        0.0
    }

    /// Encode the current EqParameters into a ParamSnapshot for widget sync.
    fn encode_snapshot(&self) -> Vec<ParamValue> {
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
            }
        }

        values
    }

    /// Apply a single GuiCommand to EqParameters.
    fn apply_command_to_params(params: &mut equaform_common::EqParameters, cmd: &GuiCommand) {
        let pid = cmd.param_id.0;
        if (1..=99).contains(&pid) {
            // Global parameters
            match pid {
                1 => params.global.bypass = cmd.value > 0.5,
                2 => params.global.output_gain_db = cmd.value.clamp(-24.0, 24.0),
                3 => params.global.auto_gain = cmd.value > 0.5,
                4 => {
                    params.global.processing_mode =
                        equaform_common::ProcessingMode::MinimumPhase;
                }
                5 => {
                    params.global.stereo_mode = equaform_common::StereoMode::Stereo;
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
        let mut eng = std::thread::Builder::new()
            .name("engine-client-init".to_string())
            .stack_size(8 * 1024 * 1024)
            .spawn(|| Box::new(EngineLifecycle::new()))
            .expect("spawn engine thread")
            .join()
            .expect("engine init panicked");

        if eng.prepare(sample_rate as f64, block_size, channels) {
            self.engine = Some(eng);

            // Initialize band_params to default 8-band musical layout.
            self.band_params = default_8_bands();
            let mut params = *self.engine.as_ref().unwrap().eq_parameters();
            params.bands = self.band_params;
            self.engine.as_mut().unwrap().set_parameters(params);

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
            // Skip read-only telemetry parameter ID 6 (auto-gain compensation readout)
            if pv.param_id.0 == 6 {
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
        self.preset_manager.iter().map(|p| p.name.clone()).collect()
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
        self.egui_context.as_ref().expect("egui context not set")
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
        // Return a reference to the latest spectrum data.
        // In Phase 6 (audio backend), this will be updated every sample block.
        // For now, return an empty slice — the analysis_snapshot() already
        // provides spectrum_data for the UI plot.
        static EMPTY: &[[f64; 2]] = &[];
        EMPTY
    }
}
