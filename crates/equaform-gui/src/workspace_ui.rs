//! Shared Workspace UI for Equaform-8.
//!
//! Provides `EquaformWorkspaceUi`, a renderer for the complete Equaform equalizer workspace
//! (dual plots, master & presets bar, 8 vertical channel strips) that works identically
//! in standalone mode and embedded inside a DAW plugin window.

use equaform_common::{BandParameters, MAX_BANDS, ParamId};

use crate::ab_compare::{AbCompareState, AbSlot};
use crate::i18n::{Language, Translations};
use crate::preset_manager::PresetManager;
use crate::presets_io::{is_factory_preset, partition_and_sort_presets};
use crate::undo_stack::{EqUndoController, capture_undo_snapshot, is_undoable_param};
use crate::{
    AnalysisSnapshot, BandControls, BandRenderContext, EqCurveCache, GuiCommandBatch,
    ParamSnapshot, PlotContext, render_band, render_interactive_frequency_response_plot,
    render_master_section_with_rate, render_spectrum_analyzer_localized,
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
    /// Change the UI display language.
    SetLanguage(Language),
}

/// State for interactive dragging, curve caching, and preset text input across UI frames.
#[derive(Default)]
pub struct WorkspaceUiState {
    /// Band currently being dragged by mouse on the EQ plot.
    pub dragged_band: Option<usize>,
    /// Name input text for saving user presets.
    pub preset_name_input: String,
    /// Active UI language.
    pub language: Language,
    /// Dirty-flag cache for the interactive EQ frequency response curves.
    pub curve_cache: EqCurveCache,
    /// A/B comparison slot state (persisted across UI frames, shared by Standalone & DAW plugins).
    pub ab_compare: AbCompareState,
    /// Bounded undo/redo history controller (shared by Standalone & DAW plugins).
    pub undo: EqUndoController,
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

/// Mutable outputs accumulated while rendering a single workspace frame.
struct FrameOutput {
    batch: GuiCommandBatch,
    actions: Vec<WorkspaceAction>,
    undo_requested: bool,
    redo_requested: bool,
}

impl FrameOutput {
    fn new() -> Self {
        Self {
            batch: GuiCommandBatch::new(),
            actions: Vec::new(),
            undo_requested: false,
            redo_requested: false,
        }
    }
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
    let mut out = FrameOutput::new();
    let t = Translations::for_language(state.language);
    let pre_frame_snapshot = capture_undo_snapshot(ctx.snapshot);

    consume_undo_shortcuts(ui, &mut out);
    ui.spacing_mut().item_spacing.y = 4.0;

    render_top_toolbar(ui, state, ctx, t, &mut out);
    ui.separator();

    render_plots_and_vu(ui, state, ctx, t, &mut out.batch);
    ui.separator();

    render_master_and_presets_bar(ui, state, ctx, t, &mut out);
    ui.separator();

    render_band_strips(ui, ctx.snapshot, t, &mut out.batch);

    finalize_undo_frame(ui, state, ctx.snapshot, pre_frame_snapshot, &mut out);
    (out.batch, out.actions)
}

fn consume_undo_shortcuts(ui: &mut egui::Ui, out: &mut FrameOutput) {
    if ui.ctx().wants_keyboard_input() {
        return;
    }
    ui.input_mut(|input| {
        let cmd_shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
        if input.consume_key(cmd_shift, egui::Key::Z)
            || input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
        {
            out.redo_requested = true;
        } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z) {
            out.undo_requested = true;
        }
    });
}

fn render_top_toolbar(
    ui: &mut egui::Ui,
    state: &mut WorkspaceUiState,
    ctx: &WorkspaceUiContext<'_>,
    t: &'static Translations,
    out: &mut FrameOutput,
) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Equaform-8")
                .strong()
                .size(15.0)
                .color(egui::Color32::from_rgb(0, 205, 255)),
        );
        ui.separator();
        render_bypass_button(ui, ctx.snapshot, t, &mut out.batch);
        ui.separator();
        render_ab_compare_controls(ui, state, ctx.snapshot, t, &mut out.batch);
        ui.separator();
        render_undo_redo_buttons(ui, state, t, out);
        ui.separator();
        render_host_controls(ui, ctx.env, t, &mut out.actions);

        if let Some(status) = ctx.status_message {
            ui.separator();
            ui.label(
                egui::RichText::new(status)
                    .small()
                    .color(egui::Color32::from_rgb(165, 175, 190)),
            );
        }

        render_language_picker(ui, state, t, &mut out.actions);
    });
}

fn render_bypass_button(
    ui: &mut egui::Ui,
    snapshot: &ParamSnapshot,
    t: &'static Translations,
    batch: &mut GuiCommandBatch,
) {
    let is_bypassed = snapshot.get(ParamId(1)).map(|v| v >= 0.5).unwrap_or(false);
    let (bypass_text, bypass_color) = if is_bypassed {
        (t.bypass_on, egui::Color32::from_rgb(175, 180, 190))
    } else {
        (t.bypass_off, egui::Color32::from_rgb(70, 235, 130))
    };

    if ui
        .add(egui::Button::new(
            egui::RichText::new(bypass_text)
                .strong()
                .color(bypass_color),
        ))
        .on_hover_text(if is_bypassed {
            t.tooltip_enable_eq
        } else {
            t.tooltip_bypass_eq
        })
        .clicked()
    {
        batch.push(ParamId(1), if is_bypassed { 0.0 } else { 1.0 });
    }
}

fn render_ab_compare_controls(
    ui: &mut egui::Ui,
    state: &mut WorkspaceUiState,
    snapshot: &ParamSnapshot,
    t: &'static Translations,
    batch: &mut GuiCommandBatch,
) {
    state.ab_compare.ensure_initialized(snapshot);
    let active_slot = state.ab_compare.active();
    let inactive_fg = egui::Color32::from_rgb(175, 180, 190);
    let active_fg = egui::Color32::from_rgb(16, 20, 26);
    let inactive_bg = egui::Color32::from_rgb(42, 46, 54);

    let slots = [
        (
            AbSlot::A,
            "A",
            egui::Color32::from_rgb(0, 205, 255),
            t.tooltip_compare_slot_a,
        ),
        (
            AbSlot::B,
            "B",
            egui::Color32::from_rgb(255, 165, 50),
            t.tooltip_compare_slot_b,
        ),
    ];
    for (slot, label, active_bg, tooltip) in slots {
        let is_active = active_slot == slot;
        let btn = egui::Button::new(egui::RichText::new(label).strong().color(if is_active {
            active_fg
        } else {
            inactive_fg
        }))
        .fill(if is_active { active_bg } else { inactive_bg })
        .min_size(egui::vec2(22.0, 18.0));
        if ui.add(btn).on_hover_text(tooltip).clicked() {
            state.ab_compare.switch_to(slot, snapshot, batch);
        }
    }

    if ui
        .add(
            egui::Button::new(
                egui::RichText::new(active_slot.copy_label())
                    .small()
                    .color(egui::Color32::from_rgb(195, 200, 210)),
            )
            .min_size(egui::vec2(34.0, 18.0)),
        )
        .on_hover_text(format!(
            "{} ({})",
            t.tooltip_copy_ab_settings,
            active_slot.copy_label()
        ))
        .clicked()
    {
        state.ab_compare.copy_active_to_other(snapshot);
    }
}

fn render_undo_redo_buttons(
    ui: &mut egui::Ui,
    state: &WorkspaceUiState,
    t: &'static Translations,
    out: &mut FrameOutput,
) {
    if ui
        .add_enabled(
            state.undo.can_undo(),
            egui::Button::new(egui::RichText::new("<-").strong()).min_size(egui::vec2(26.0, 18.0)),
        )
        .on_hover_text(t.tooltip_undo)
        .clicked()
    {
        out.undo_requested = true;
    }
    if ui
        .add_enabled(
            state.undo.can_redo(),
            egui::Button::new(egui::RichText::new("->").strong()).min_size(egui::vec2(26.0, 18.0)),
        )
        .on_hover_text(t.tooltip_redo)
        .clicked()
    {
        out.redo_requested = true;
    }
}

fn render_host_controls(
    ui: &mut egui::Ui,
    env: &HostEnvironment,
    t: &'static Translations,
    actions: &mut Vec<WorkspaceAction>,
) {
    match env {
        HostEnvironment::Standalone {
            audio_active,
            current_output,
            available_outputs,
            ..
        } => {
            let mute_label = if *audio_active { t.mute } else { t.unmute };
            if ui.button(mute_label).clicked() {
                actions.push(WorkspaceAction::TogglePlayStop);
            }
            ui.separator();
            ui.label(t.output_label);
            egui::ComboBox::from_id_salt("output_device_select")
                .selected_text(current_output.as_str())
                .show_ui(ui, |ui| {
                    for dev in available_outputs {
                        let is_selected = dev == current_output;
                        if ui.selectable_label(is_selected, dev.as_str()).clicked() && !is_selected
                        {
                            actions.push(WorkspaceAction::SelectOutputDevice(dev.clone()));
                        }
                    }
                });
            if ui
                .button("🔄")
                .on_hover_text(t.tooltip_refresh_devices)
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
                    "🎛 {host_description} • {:.1} kHz • {}",
                    host_sr / 1000.0,
                    t.stereo_label
                ))
                .small()
                .color(egui::Color32::from_rgb(170, 180, 195)),
            );
        }
    }
}

fn render_language_picker(
    ui: &mut egui::Ui,
    state: &mut WorkspaceUiState,
    t: &'static Translations,
    actions: &mut Vec<WorkspaceAction>,
) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let mut chosen_lang = state.language;
        egui::ComboBox::from_id_salt("language_select")
            .width(52.0)
            .selected_text(state.language.short_code())
            .show_ui(ui, |ui| {
                for lang in Language::ALL {
                    ui.selectable_value(&mut chosen_lang, lang, lang.native_name());
                }
            })
            .response
            .on_hover_text(t.tooltip_language);

        if chosen_lang != state.language {
            state.language = chosen_lang;
            actions.push(WorkspaceAction::SetLanguage(chosen_lang));
        }
    });
}

fn render_plots_and_vu(
    ui: &mut egui::Ui,
    state: &mut WorkspaceUiState,
    ctx: &WorkspaceUiContext<'_>,
    t: &'static Translations,
    batch: &mut GuiCommandBatch,
) {
    let (peak_l, peak_r) = match ctx.env {
        HostEnvironment::Standalone { peak_levels_db, .. }
        | HostEnvironment::Plugin { peak_levels_db, .. } => *peak_levels_db,
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
                        ui.label(egui::RichText::new(t.eq_response_title).strong());
                        ui.label(
                            egui::RichText::new(t.eq_response_hint)
                                .small()
                                .color(egui::Color32::from_rgb(155, 160, 170)),
                        );
                    });
                    render_interactive_frequency_response_plot(
                        &mut cols[0],
                        PlotContext {
                            bands: ctx.band_params,
                            sample_rate: ctx.sample_rate,
                            width: 512,
                            translations: t,
                        },
                        &mut state.curve_cache,
                        &mut state.dragged_band,
                        batch,
                    );

                    cols[1].horizontal(|ui| {
                        ui.label(egui::RichText::new(t.spectrum_title).strong());
                    });
                    render_spectrum_analyzer_localized(
                        &mut cols[1],
                        ctx.analysis,
                        ctx.sample_rate,
                        128,
                        t,
                    );
                });
            },
        );
        ui.separator();
        render_vertical_stereo_vu_meter(ui, peak_l, peak_r, 180.0);
    });
}

fn render_master_and_presets_bar(
    ui: &mut egui::Ui,
    state: &mut WorkspaceUiState,
    ctx: &WorkspaceUiContext<'_>,
    t: &'static Translations,
    out: &mut FrameOutput,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        render_master_section_with_rate(ui, ctx.snapshot, ctx.sample_rate, t, &mut out.batch);
        ui.separator();

        ui.label(egui::RichText::new(t.preset_label).strong());
        let (user_presets, factory_presets) =
            partition_and_sort_presets(ctx.preset_manager.iter().map(|p| p.name.as_str()));
        let selected_text = if ctx.current_preset.is_empty() {
            t.select_preset
        } else {
            ctx.current_preset
        };

        egui::ComboBox::new("preset_select", "")
            .width(145.0)
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                render_preset_list(ui, &user_presets, ctx.current_preset, &mut out.actions);
                if !user_presets.is_empty() && !factory_presets.is_empty() {
                    ui.separator();
                }
                render_preset_list(ui, &factory_presets, ctx.current_preset, &mut out.actions);
            });

        render_preset_save_delete(ui, state, ctx, t, &mut out.actions);
    });
}

fn render_preset_list(
    ui: &mut egui::Ui,
    presets: &[String],
    current_preset: &str,
    actions: &mut Vec<WorkspaceAction>,
) {
    for name in presets {
        let is_selected = name.as_str() == current_preset;
        if ui.selectable_label(is_selected, name.as_str()).clicked() && !is_selected {
            actions.push(WorkspaceAction::ApplyPreset(name.clone()));
        }
    }
}

fn render_preset_save_delete(
    ui: &mut egui::Ui,
    state: &mut WorkspaceUiState,
    ctx: &WorkspaceUiContext<'_>,
    t: &'static Translations,
    actions: &mut Vec<WorkspaceAction>,
) {
    ui.add(
        egui::TextEdit::singleline(&mut state.preset_name_input)
            .desired_width(100.0)
            .hint_text(t.preset_name_hint),
    );

    if ui.button(t.save_button).clicked() {
        let name = if !state.preset_name_input.trim().is_empty() {
            state.preset_name_input.trim().to_string()
        } else if !ctx.current_preset.trim().is_empty() {
            ctx.current_preset.trim().to_string()
        } else {
            ctx.preset_manager.next_auto_name()
        };
        actions.push(WorkspaceAction::SavePreset(name));
        state.preset_name_input.clear();
    }

    let is_factory = is_factory_preset(ctx.current_preset);
    let can_delete = !ctx.current_preset.is_empty() && !is_factory;
    if ui
        .add_enabled(can_delete, egui::Button::new(t.delete_button))
        .on_hover_text(if is_factory {
            t.tooltip_factory_preset_locked
        } else {
            t.tooltip_delete_preset
        })
        .clicked()
    {
        actions.push(WorkspaceAction::DeletePreset(
            ctx.current_preset.to_string(),
        ));
    }
}

fn render_band_strips(
    ui: &mut egui::Ui,
    snapshot: &ParamSnapshot,
    t: &'static Translations,
    batch: &mut GuiCommandBatch,
) {
    ui.columns(8, |cols| {
        let band_controls = BandControls::new();
        for (i, col) in cols.iter_mut().enumerate() {
            let bp = band_controls.params(i);
            render_band(
                col,
                BandRenderContext {
                    band_index: i,
                    params: &bp,
                    snapshot,
                    translations: t,
                },
                batch,
            );
        }
    });
}

fn finalize_undo_frame(
    ui: &egui::Ui,
    state: &mut WorkspaceUiState,
    snapshot: &ParamSnapshot,
    pre_frame_snapshot: Vec<(equaform_common::ParamId, f32)>,
    out: &mut FrameOutput,
) {
    let restored_from_history = if out.undo_requested {
        state.undo.undo(snapshot, &mut out.batch)
    } else if out.redo_requested {
        state.undo.redo(snapshot, &mut out.batch)
    } else {
        false
    };

    if !restored_from_history {
        let changed_this_frame = out
            .batch
            .commands()
            .iter()
            .any(|cmd| is_undoable_param(cmd.param_id))
            || out
                .actions
                .iter()
                .any(|action| matches!(action, WorkspaceAction::ApplyPreset(_)));
        let pointer_down = ui.input(|i| i.pointer.any_down()) || state.dragged_band.is_some();
        state
            .undo
            .observe_frame(pre_frame_snapshot, changed_this_frame, pointer_down);
    }
}
