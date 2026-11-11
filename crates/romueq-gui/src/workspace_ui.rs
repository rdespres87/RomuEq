//! Shared Workspace UI for RomuEq.
//!
//! Provides `RomuEqWorkspaceUi`, a renderer for the complete RomuEq equalizer workspace
//! (dual plots, master & presets bar, 8 vertical channel strips) that works identically
//! in standalone mode and embedded inside a DAW plugin window.

use romueq_common::{BandParameters, MAX_BANDS, ParamId};

use crate::preset_manager::PresetManager;
use crate::{
    AnalysisSnapshot, BandControls, GuiCommandBatch, ParamSnapshot, render_band,
    render_interactive_frequency_response_plot, render_master_section, render_spectrum_analyzer,
    render_stereo_vu_meter,
};

/// Host environment under which the workspace UI is currently rendered.
#[derive(Debug, Clone)]
pub enum HostEnvironment {
    /// Standalone application mode with audio stream control.
    Standalone {
        /// Whether audio playback is currently active.
        audio_active: bool,
        /// Currently selected CoreAudio output device name.
        current_output: String,
        /// List of available output device names.
        available_outputs: Vec<String>,
        /// Peak stereo levels (L, R) in dBFS.
        peak_levels_db: (f32, f32),
    },
    /// Embedded inside a DAW plugin window (VST3, CLAP, AUv2).
    Plugin {
        /// Host DAW name or plugin format description (e.g. "Ableton Live • VST3").
        host_description: String,
        /// Current processing sample rate reported by the host DAW.
        sample_rate: f32,
        /// Peak stereo levels (L, R) in dBFS computed from audio processing blocks.
        peak_levels_db: (f32, f32),
    },
}

/// Actions emitted by the workspace UI for the hosting container to handle.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceAction {
    /// Toggle standalone audio play/stop.
    TogglePlayStop,
    /// Change the standalone CoreAudio output device.
    SelectOutputDevice(String),
    /// Refresh the standalone audio device list.
    RefreshDevices,
    /// Apply a named preset.
    ApplyPreset(String),
    /// Save a preset under the specified name.
    SavePreset(String),
    /// Delete a named preset.
    DeletePreset(String),
}

/// State for interactive dragging and preset text input across UI frames.
#[derive(Default)]
pub struct WorkspaceUiState {
    /// Band currently being dragged by mouse on the EQ plot.
    pub dragged_band: Option<usize>,
    /// Name input text for saving user presets.
    pub preset_name_input: String,
}

/// Renders the complete RomuEq workspace UI.
///
/// Returns a tuple of:
/// - `GuiCommandBatch`: DSP parameter adjustments made by the user during this frame.
/// - `Vec<WorkspaceAction>`: High-level actions (preset save/load, playback, device selection).
#[allow(clippy::too_many_arguments)]
pub fn render_workspace(
    ui: &mut egui::Ui,
    state: &mut WorkspaceUiState,
    env: &HostEnvironment,
    band_params: &[BandParameters; MAX_BANDS],
    sample_rate: f32,
    snapshot: &ParamSnapshot,
    analysis: &AnalysisSnapshot,
    preset_manager: &PresetManager,
    current_preset: &str,
    status_message: Option<&str>,
) -> (GuiCommandBatch, Vec<WorkspaceAction>) {
    let mut batch = GuiCommandBatch::new();
    let mut actions = Vec::new();

    ui.spacing_mut().item_spacing.y = 4.0;

    // 1. Compact header row: Title + Status message
    ui.horizontal(|ui| {
        ui.heading("RomuEq — Égaliseur Paramétrique");
        if let Some(status) = status_message {
            ui.separator();
            ui.label(
                egui::RichText::new(status)
                    .small()
                    .color(egui::Color32::from_rgb(165, 175, 190)),
            );
        }
    });

    // 2. Top toolbar (Bypass + Master VU + Host/Device info)
    ui.horizontal(|ui| {
        match env {
            HostEnvironment::Standalone {
                audio_active,
                current_output,
                available_outputs,
                peak_levels_db,
            } => {
                let play_label = if *audio_active {
                    "⏹ Stop Audio"
                } else {
                    "▶ Play Audio"
                };
                if ui.button(play_label).clicked() {
                    actions.push(WorkspaceAction::TogglePlayStop);
                }

                // Equalizer On / Off (Direct Bypass) toggle button
                let is_bypassed = snapshot.get(ParamId(1)).map(|v| v >= 0.5).unwrap_or(false);
                let eq_text = if is_bypassed {
                    "⚡ ÉGALISEUR : OFF (BYPASS)"
                } else {
                    "✨ ÉGALISEUR : ON"
                };
                let eq_color = if is_bypassed {
                    egui::Color32::from_rgb(180, 180, 180)
                } else {
                    egui::Color32::from_rgb(80, 240, 120)
                };

                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(eq_text).strong().color(eq_color),
                    ))
                    .on_hover_text(if is_bypassed {
                        "Cliquez pour activer le traitement d'égalisation (ON)"
                    } else {
                        "Cliquez pour passer en flux direct transparent sans traitement (BYPASS DIRECT)"
                    })
                    .clicked()
                {
                    batch.push(ParamId(1), if is_bypassed { 0.0 } else { 1.0 });
                }

                ui.separator();

                let mut chosen_out = current_output.clone();
                ui.label("🔊 Sortie:");
                egui::ComboBox::from_id_salt("output_device_select")
                    .selected_text(current_output)
                    .show_ui(ui, |ui| {
                        for dev in available_outputs {
                            ui.selectable_value(&mut chosen_out, dev.clone(), dev.as_str());
                        }
                    });

                if chosen_out != *current_output {
                    actions.push(WorkspaceAction::SelectOutputDevice(chosen_out));
                }

                if ui
                    .button("🔄")
                    .on_hover_text("Actualiser les périphériques audio")
                    .clicked()
                {
                    actions.push(WorkspaceAction::RefreshDevices);
                }

                ui.separator();
                render_stereo_vu_meter(ui, peak_levels_db.0, peak_levels_db.1);
            }
            HostEnvironment::Plugin {
                host_description,
                sample_rate: host_sr,
                peak_levels_db,
            } => {
                // Equalizer On / Off (Direct Bypass) toggle button
                let is_bypassed = snapshot.get(ParamId(1)).map(|v| v >= 0.5).unwrap_or(false);
                let eq_text = if is_bypassed {
                    "⚡ ÉGALISEUR : OFF (BYPASS)"
                } else {
                    "✨ ÉGALISEUR : ON"
                };
                let eq_color = if is_bypassed {
                    egui::Color32::from_rgb(180, 180, 180)
                } else {
                    egui::Color32::from_rgb(80, 240, 120)
                };

                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(eq_text).strong().color(eq_color),
                    ))
                    .on_hover_text("Bypass de l'égaliseur dans le DAW")
                    .clicked()
                {
                    batch.push(ParamId(1), if is_bypassed { 0.0 } else { 1.0 });
                }

                ui.separator();

                ui.label(
                    egui::RichText::new(format!(
                        "🎛 {host_description} • {:.1} kHz • Stéréo",
                        host_sr / 1000.0
                    ))
                    .small()
                    .color(egui::Color32::from_rgb(170, 180, 195)),
                );

                ui.separator();
                render_stereo_vu_meter(ui, peak_levels_db.0, peak_levels_db.1);
            }
        }
    });

    ui.separator();

    // 3. Two side-by-side plots: Left = Equalizer Curve, Right = Real-Time FFT Spectrum
    ui.columns(2, |cols| {
        cols[0].horizontal(|ui| {
            ui.label(egui::RichText::new("📈 Courbe d'Égalisation (dB)").strong());
            ui.label(
                egui::RichText::new("— Glisser: Freq/Gain • Molette: Q")
                    .small()
                    .color(egui::Color32::from_rgb(155, 160, 170)),
            );
        });
        render_interactive_frequency_response_plot(
            &mut cols[0],
            band_params,
            sample_rate,
            512,
            &mut state.dragged_band,
            &mut batch,
        );

        cols[1].horizontal(|ui| {
            ui.label(egui::RichText::new("📊 Spectre FFT Temps-Réel (dBFS)").strong());
        });
        render_spectrum_analyzer(&mut cols[1], analysis, sample_rate, 128);
    });

    ui.separator();

    // 4. Unified Master & Preset bar on a single compact row
    ui.horizontal(|ui| {
        render_master_section(ui, snapshot, &mut batch);

        ui.separator();

        ui.label(egui::RichText::new("Preset:").strong());
        let presets: Vec<String> = preset_manager.iter().map(|p| p.name.clone()).collect();
        let selected_text = if current_preset.is_empty() {
            "Choisir un preset".to_string()
        } else {
            current_preset.to_string()
        };

        let mut chosen_preset = current_preset.to_string();
        egui::ComboBox::new("preset_select", "")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for name in &presets {
                    ui.selectable_value(&mut chosen_preset, name.clone(), name.as_str());
                }
            });

        if chosen_preset != current_preset && !chosen_preset.is_empty() {
            actions.push(WorkspaceAction::ApplyPreset(chosen_preset));
        }

        ui.add(
            egui::TextEdit::singleline(&mut state.preset_name_input)
                .desired_width(110.0)
                .hint_text("Nom du preset..."),
        );

        if ui.button("💾 Sauvegarder").clicked() {
            let name = if !state.preset_name_input.trim().is_empty() {
                state.preset_name_input.trim().to_string()
            } else if !current_preset.trim().is_empty() {
                current_preset.trim().to_string()
            } else {
                preset_manager.next_auto_name()
            };
            actions.push(WorkspaceAction::SavePreset(name));
            state.preset_name_input.clear();
        }

        if ui.button("🗑 Supprimer").clicked() && !current_preset.is_empty() {
            actions.push(WorkspaceAction::DeletePreset(current_preset.to_string()));
        }
    });

    ui.separator();

    // 5. 8 equal-width columns Channel Strip grid
    ui.columns(8, |cols| {
        let band_controls = BandControls::new();
        for (i, col) in cols.iter_mut().enumerate() {
            let bp = band_controls.params(i);
            render_band(col, i, &bp, snapshot, &mut batch);
        }
    });

    (batch, actions)
}
