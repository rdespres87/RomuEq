//! Shared Workspace UI for Equaform-8.
//!
//! Provides `EquaformWorkspaceUi`, a renderer for the complete Equaform equalizer workspace
//! (dual plots, master & presets bar, 8 vertical channel strips) that works identically
//! in standalone mode and embedded inside a DAW plugin window.

use equaform_common::{BandParameters, MAX_BANDS, ParamId};

use crate::preset_manager::PresetManager;
use crate::presets_io::is_factory_preset;
use crate::{
    AnalysisSnapshot, BandControls, GuiCommandBatch, ParamSnapshot, render_band,
    render_interactive_frequency_response_plot, render_master_section, render_spectrum_analyzer,
    render_vertical_stereo_vu_meter,
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

/// Context parameters required to render the Equaform workspace UI.
pub struct WorkspaceUiContext<'a> {
    pub env: &'a HostEnvironment,
    pub band_params: &'a [BandParameters; MAX_BANDS],
    pub sample_rate: f32,
    pub snapshot: &'a ParamSnapshot,
    pub analysis: &'a AnalysisSnapshot,
    pub preset_manager: &'a PresetManager,
    pub current_preset: &'a str,
    pub status_message: Option<&'a str>,
}

/// Renders the complete Equaform workspace UI.
///
/// Returns a tuple of:
/// - `GuiCommandBatch`: DSP parameter adjustments made by the user during this frame.
/// - `Vec<WorkspaceAction>`: High-level actions (preset save/load, playback, device selection).
pub fn render_workspace(
    ui: &mut egui::Ui,
    state: &mut WorkspaceUiState,
    ctx: &WorkspaceUiContext<'_>,
) -> (GuiCommandBatch, Vec<WorkspaceAction>) {
    let env = ctx.env;
    let band_params = ctx.band_params;
    let sample_rate = ctx.sample_rate;
    let snapshot = ctx.snapshot;
    let analysis = ctx.analysis;
    let preset_manager = ctx.preset_manager;
    let current_preset = ctx.current_preset;
    let status_message = ctx.status_message;

    let mut batch = GuiCommandBatch::new();
    let mut actions = Vec::new();

    ui.spacing_mut().item_spacing.y = 4.0;

    // 1. Top toolbar: Brand + Bypass + Output/Host info + Status
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Equaform-8")
                .strong()
                .size(15.0)
                .color(egui::Color32::from_rgb(0, 205, 255)),
        );

        ui.separator();

        // Sleek unified Bypass button (LED style)
        let is_bypassed = snapshot.get(ParamId(1)).map(|v| v >= 0.5).unwrap_or(false);
        let (bypass_text, bypass_color) = if is_bypassed {
            ("⏻ BYPASS", egui::Color32::from_rgb(175, 180, 190))
        } else {
            ("⏻ ACTIVE", egui::Color32::from_rgb(70, 235, 130))
        };

        if ui
            .add(egui::Button::new(
                egui::RichText::new(bypass_text)
                    .strong()
                    .color(bypass_color),
            ))
            .on_hover_text(if is_bypassed {
                "Click to enable EQ processing"
            } else {
                "Click to bypass EQ (dry pass-through)"
            })
            .clicked()
        {
            batch.push(ParamId(1), if is_bypassed { 0.0 } else { 1.0 });
        }

        ui.separator();

        match env {
            HostEnvironment::Standalone {
                audio_active,
                current_output,
                available_outputs,
                ..
            } => {
                let mute_label = if *audio_active { "Mute" } else { "Unmute" };
                if ui.button(mute_label).clicked() {
                    actions.push(WorkspaceAction::TogglePlayStop);
                }

                ui.separator();

                let mut chosen_out = current_output.clone();
                ui.label("🔊 Output:");
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
                    .on_hover_text("Refresh audio devices")
                    .clicked()
                {
                    actions.push(WorkspaceAction::RefreshDevices);
                }
            }
            HostEnvironment::Plugin {
                host_description,
                sample_rate: host_sr,
                ..
            } => {
                ui.label(
                    egui::RichText::new(format!(
                        "🎛 {host_description} • {:.1} kHz • Stereo",
                        host_sr / 1000.0
                    ))
                    .small()
                    .color(egui::Color32::from_rgb(170, 180, 195)),
                );
            }
        }

        if let Some(status) = status_message {
            ui.separator();
            ui.label(
                egui::RichText::new(status)
                    .small()
                    .color(egui::Color32::from_rgb(165, 175, 190)),
            );
        }
    });

    ui.separator();

    // 2. Central section: Two side-by-side plots + Vertical stereo VU-meter on the right
    let (peak_l, peak_r) = match env {
        HostEnvironment::Standalone { peak_levels_db, .. } => *peak_levels_db,
        HostEnvironment::Plugin { peak_levels_db, .. } => *peak_levels_db,
    };

    ui.horizontal(|ui| {
        let meter_w = 47.0;
        let right_section_w = meter_w + ui.spacing().item_spacing.x * 2.0 + 7.0;
        let plots_w = (ui.available_width() - right_section_w).max(200.0);
        ui.allocate_ui_with_layout(
            egui::vec2(plots_w, 185.0),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                ui.columns(2, |cols| {
                    cols[0].horizontal(|ui| {
                        ui.label(egui::RichText::new("📈 EQ Response (dB)").strong());
                        ui.label(
                            egui::RichText::new("— Drag: Freq/Gain • Scroll: Q")
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
                        ui.label(egui::RichText::new("📊 Real-Time Spectrum (dBFS)").strong());
                    });
                    render_spectrum_analyzer(&mut cols[1], analysis, sample_rate, 128);
                });
            },
        );

        ui.separator();

        render_vertical_stereo_vu_meter(ui, peak_l, peak_r, 180.0);
    });

    ui.separator();

    // 3. Unified Master & Preset bar on a single compact row
    ui.horizontal(|ui| {
        render_master_section(ui, snapshot, &mut batch);

        ui.separator();

        ui.label(egui::RichText::new("Preset:").strong());
        let presets: Vec<String> = preset_manager.iter().map(|p| p.name.clone()).collect();
        let selected_text = if current_preset.is_empty() {
            "Select Preset...".to_string()
        } else {
            current_preset.to_string()
        };

        let mut chosen_preset = current_preset.to_string();
        egui::ComboBox::new("preset_select", "")
            .width(170.0)
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
                .desired_width(130.0)
                .hint_text("Preset name..."),
        );

        if ui.button("💾 Save").clicked() {
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

        let is_factory = is_factory_preset(current_preset);
        let can_delete = !current_preset.is_empty() && !is_factory;
        if ui
            .add_enabled(can_delete, egui::Button::new("🗑 Delete"))
            .on_hover_text(if is_factory {
                "Factory presets cannot be deleted"
            } else {
                "Delete selected preset"
            })
            .clicked()
        {
            actions.push(WorkspaceAction::DeletePreset(current_preset.to_string()));
        }
    });

    ui.separator();

    // 4. 8 equal-width columns Channel Strip grid
    ui.columns(8, |cols| {
        let band_controls = BandControls::new();
        for (i, col) in cols.iter_mut().enumerate() {
            let bp = band_controls.params(i);
            render_band(col, i, &bp, snapshot, &mut batch);
        }
    });

    (batch, actions)
}
