//! Standalone desktop runner for Equaform (eframe/winit + CoreAudio/CPAL).

mod engine_client;
use engine_client::EngineClient;

use crate::*;
use eframe::App;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static ACTIVE_EGUI_CTX: OnceLock<egui::Context> = OnceLock::new();
static GRACEFUL_SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Request a clean GUI + audio shutdown identical to closing the application window.
///
/// Safe to call from any non-signal thread (e.g. a POSIX self-pipe signal watcher).
/// Returns `true` if the active eframe viewport was notified to close.
pub fn request_graceful_shutdown() -> bool {
    GRACEFUL_SHUTDOWN_REQUESTED.store(true, Ordering::Release);
    if let Some(ctx) = ACTIVE_EGUI_CTX.get() {
        ctx.request_repaint();
        true
    } else {
        false
    }
}

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
    let saved_lang = Language::load_saved();
    let window_title = Translations::for_language(saved_lang)
        .window_title
        .to_string();
    eframe::run_native(
        &window_title,
        options,
        Box::new(move |cc| {
            let _ = ACTIVE_EGUI_CTX.set(cc.egui_ctx.clone());
            let ppp = cc.egui_ctx.pixels_per_point();
            cc.egui_ctx.set_pixels_per_point(ppp);
            if GRACEFUL_SHUTDOWN_REQUESTED.load(Ordering::Acquire) {
                cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
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
    fn new(mut app: A, ctx: egui::Context) -> Self {
        let mut client = EngineClient::new(ctx);
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

        let saved_lang = Language::load_saved();
        let t = Translations::for_language(saved_lang);
        let initial_bypassed = client
            .engine
            .as_ref()
            .map(|e| e.eq_parameters().global.bypass)
            .unwrap_or(false);
        let (audio_active, status_message) = match audio_stream.start() {
            Ok(()) => {
                app.on_audio_started();
                client.sample_rate = audio_stream.sample_rate();
                let out_name = detected_out.as_deref().unwrap_or(t.output_slider);
                (
                    true,
                    Some(format_audio_status(initial_bypassed, out_name, t)),
                )
            }
            Err(e) => (false, Some(format!("{}: {e}", t.status_stream_error))),
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
            ui_state: WorkspaceUiState {
                language: saved_lang,
                ..Default::default()
            },
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
        if GRACEFUL_SHUTDOWN_REQUESTED.load(Ordering::Acquire) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
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
        let t = Translations::for_language(self.ui_state.language);
        let env = HostEnvironment::Standalone {
            audio_active: self.audio_active,
            current_output: self
                .audio_stream
                .as_ref()
                .and_then(|a| a.selected_output_device())
                .unwrap_or(t.system_default_output)
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
        let t = Translations::for_language(self.ui_state.language);
        for action in actions {
            match action {
                WorkspaceAction::TogglePlayStop => {
                    if let Some(ref mut audio) = self.audio_stream {
                        if audio.is_active() {
                            let now_muted = !audio.is_muted();
                            audio.set_muted(now_muted);
                            self.audio_active = !now_muted;
                            if now_muted {
                                self.status_message = Some(t.status_audio_muted.to_string());
                            } else {
                                let out_name =
                                    audio.selected_output_device().unwrap_or(t.output_slider);
                                let is_bypassed = self
                                    .client
                                    .snapshot()
                                    .get(ParamId(1))
                                    .map(|v| v >= 0.5)
                                    .unwrap_or(false);
                                self.status_message =
                                    Some(format_audio_status(is_bypassed, out_name, t));
                            }
                        } else {
                            audio.set_muted(false);
                            if let Some(ref engine) = self.client.engine {
                                let _ = audio.set_parameters(*engine.eq_parameters());
                            }
                            match audio.start() {
                                Ok(()) => {
                                    self.audio_active = true;
                                    self.client.sample_rate = audio.sample_rate();
                                    let out_name =
                                        audio.selected_output_device().unwrap_or(t.output_slider);
                                    let is_bypassed = self
                                        .client
                                        .snapshot()
                                        .get(ParamId(1))
                                        .map(|v| v >= 0.5)
                                        .unwrap_or(false);
                                    self.status_message =
                                        Some(format_audio_status(is_bypassed, out_name, t));
                                }
                                Err(err) => {
                                    self.status_message =
                                        Some(format!("{}: {err}", t.status_audio_error));
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
                                    let is_bypassed = self
                                        .client
                                        .snapshot()
                                        .get(ParamId(1))
                                        .map(|v| v >= 0.5)
                                        .unwrap_or(false);
                                    self.status_message =
                                        Some(format_audio_status(is_bypassed, &dev, t));
                                }
                                Err(e) => {
                                    self.audio_active = false;
                                    self.status_message =
                                        Some(format!("{} '{dev}': {e}", t.status_output_error));
                                }
                            }
                        } else {
                            self.status_message =
                                Some(format!("{}: {dev}", t.status_selected_output));
                        }
                    }
                }
                WorkspaceAction::RefreshDevices => {
                    self.available_outputs = audio::AudioStream::available_output_devices();
                    self.available_inputs = audio::AudioStream::available_input_devices();
                    self.status_message = Some(t.status_devices_refreshed.to_string());
                }
                WorkspaceAction::ApplyPreset(name) => {
                    let _ = self.client.apply_preset(&name);
                    self.client._current_preset = name.clone();
                    save_last_active_preset_name(&name);
                    save_complete_app_state(&self.client);
                    if let Some(ref mut audio) = self.audio_stream {
                        if let Some(ref engine) = self.client.engine {
                            let _ = audio.set_parameters(*engine.eq_parameters());
                        }
                    }
                    self.status_message = Some(format!("{}: '{name}'", t.status_preset_applied));
                }
                WorkspaceAction::SavePreset(name) => {
                    if let Err(e) = self.client.create_preset(&name) {
                        self.status_message =
                            Some(format!("{} '{name}': {e}", t.status_preset_error));
                    } else {
                        self.client._current_preset = name.clone();
                        let _ = persist_preset_to_disk(&mut self.client.preset_manager, &name);
                        save_last_active_preset_name(&name);
                        save_complete_app_state(&self.client);
                        self.status_message = Some(format!("{}: '{name}'", t.status_preset_saved));
                    }
                }
                WorkspaceAction::DeletePreset(name) => {
                    let _ = self.client.remove_preset(&name);
                    remove_preset_from_disk(&name);
                    self.client._current_preset.clear();
                    save_complete_app_state(&self.client);
                    self.status_message = Some(format!("{}: '{name}'", t.status_preset_deleted));
                }
                WorkspaceAction::SetLanguage(lang) => {
                    lang.save();
                    let updated_t = Translations::for_language(lang);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                        updated_t.window_title.to_string(),
                    ));
                    if self.audio_active {
                        let out_name = self
                            .audio_stream
                            .as_ref()
                            .and_then(|a| a.selected_output_device())
                            .unwrap_or(updated_t.output_slider);
                        let is_bypassed = self
                            .client
                            .snapshot()
                            .get(ParamId(1))
                            .map(|v| v >= 0.5)
                            .unwrap_or(false);
                        self.status_message =
                            Some(format_audio_status(is_bypassed, out_name, updated_t));
                    } else if self
                        .audio_stream
                        .as_ref()
                        .map(|a| a.is_muted())
                        .unwrap_or(false)
                    {
                        self.status_message = Some(updated_t.status_audio_muted.to_string());
                    }
                }
            }
        }

        // Process the batch through the engine client.
        if !batch.is_empty() {
            let modifies_bypass = batch.commands().iter().any(|c| c.param_id.0 == 1);
            self.client.apply_commands(batch);
            save_complete_app_state(&self.client);
            if let Some(ref mut audio) = self.audio_stream {
                if let Some(ref engine) = self.client.engine {
                    let _ = audio.set_parameters(*engine.eq_parameters());
                }
            }
            if modifies_bypass && self.audio_active {
                let updated_t = Translations::for_language(self.ui_state.language);
                let out_name = self
                    .audio_stream
                    .as_ref()
                    .and_then(|a| a.selected_output_device())
                    .unwrap_or(updated_t.output_slider);
                let is_bypassed = self
                    .client
                    .snapshot()
                    .get(ParamId(1))
                    .map(|v| v >= 0.5)
                    .unwrap_or(false);
                self.status_message = Some(format_audio_status(is_bypassed, out_name, updated_t));
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
    let _ = equaform_common::state::atomic_write(path, dev.as_bytes());
}

fn load_saved_output_device() -> Option<String> {
    let path = user_config_dir().join("last_output_device.txt");
    equaform_common::state::read_to_string_bounded(
        path,
        equaform_common::state::MAX_TEXT_FILE_BYTES,
    )
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

fn save_complete_app_state(client: &EngineClient) {
    let mut state = equaform_common::state::ProjectState::new();
    for pv in client.encode_snapshot() {
        if equaform_common::parameter::parameter_info(pv.param_id.0).is_none() {
            continue;
        }
        state.set(pv.param_id.0, pv.value);
    }
    let bytes = state.serialize();
    let path = user_config_dir().join("app_state.bin");
    let _ = equaform_common::state::atomic_write(path, &bytes);
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

/// Formats the standalone status line reflecting both bypass state and active output device.
fn format_audio_status(is_bypassed: bool, out_name: &str, t: &Translations) -> String {
    let label = if is_bypassed {
        t.status_eq_bypassed
    } else {
        t.status_eq_active
    };
    format!("{label} • {out_name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use equaform_engine::EngineLifecycle;

    #[test]
    fn test_format_audio_status_reflects_bypass_state() {
        let fr = Translations::for_language(Language::French);
        assert_eq!(
            format_audio_status(false, "Komplete Audio 6 MK2", fr),
            "EQ actif • Komplete Audio 6 MK2"
        );
        assert_eq!(
            format_audio_status(true, "Komplete Audio 6 MK2", fr),
            "EQ contourné • Komplete Audio 6 MK2"
        );

        let en = Translations::for_language(Language::English);
        assert_eq!(
            format_audio_status(true, "Built-in", en),
            "EQ bypassed • Built-in"
        );
    }

    #[test]
    fn test_apply_preset_preserves_bypass_state() {
        let mut client = EngineClient::new(egui::Context::default());
        let mut engine = EngineLifecycle::new();
        let _ = engine.prepare(48_000.0, 512, 2);
        client.engine = Some(Box::new(engine));
        crate::presets_io::populate_factory_presets(&mut client.preset_manager);

        let mut bypass_on = GuiCommandBatch::new();
        bypass_on.push(ParamId(1), 1.0);
        client.apply_commands(bypass_on);
        assert_eq!(client.snapshot().get(ParamId(1)), Some(1.0));

        let preset_name = client
            .list_presets()
            .into_iter()
            .find(|n| n != "Flat")
            .unwrap_or_else(|| "Flat".to_string());
        assert!(client.apply_preset(&preset_name).is_ok());
        assert_eq!(
            client.snapshot().get(ParamId(1)),
            Some(1.0),
            "Applying a preset must preserve bypass state"
        );
    }

    #[test]
    fn test_engine_client_prepare_and_context_without_panics() {
        let mut client = EngineClient::new(egui::Context::default());
        // Context is always available by construction without set_egui_context().
        let _ = client.egui_context().pixels_per_point();

        client.prepare(48_000.0, 512, 2);
        assert!(client.engine.is_some());
        assert_eq!(client.sample_rate, 48_000.0);
        assert_eq!(client.snapshot().get(ParamId(1)), Some(0.0));

        // Re-preparing tears down and re-initializes the engine cleanly.
        client.prepare(44_100.0, 256, 2);
        assert!(client.engine.is_some());
        assert_eq!(client.sample_rate, 44_100.0);
    }

    #[test]
    fn test_standalone_create_preset_roundtrips_through_disk()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut client = EngineClient::new(egui::Context::default());
        client.prepare(48_000.0, 256, 2);

        let created = client.create_preset("HS7").map_err(std::io::Error::other)?;
        assert_eq!(created, "HS7");

        let tmp = std::env::temp_dir().join(format!(
            "equaform_standalone_hs7_{}.req",
            std::process::id()
        ));
        let path = tmp.to_str().ok_or("non-UTF8 temp path")?;
        client
            .save_preset_to_file("HS7", path)
            .map_err(std::io::Error::other)?;

        let mut fresh_manager = crate::preset_manager::PresetManager::new();
        let loaded = fresh_manager.load_from_file(path)?;
        assert_eq!(loaded.name, "HS7");
        assert_eq!(loaded.preset.parameter_values.get(6), None);
        for band_idx in 0..equaform_common::MAX_BANDS {
            let solo_id = 100u32 + band_idx as u32 * 100 + 6;
            assert_eq!(loaded.preset.parameter_values.get(solo_id), None);
        }

        let _ = std::fs::remove_file(&tmp);
        Ok(())
    }
}
