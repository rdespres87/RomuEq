//! Master output gain, auto-gain, and processing mode control bar widget.

use crate::i18n::{Language, Translations};
use crate::{GuiCommandBatch, ParamId, ParamSnapshot};
use equaform_common::{ProcessingMode, global};

const DEFAULT_TOOLTIP_SAMPLE_RATE: f32 = 48_000.0;

/// Formats a localized hover tooltip for a [`ProcessingMode`] button with sample
/// and millisecond latency at `sample_rate` (falling back to 48 kHz when invalid).
pub fn format_processing_mode_tooltip(
    desc: &str,
    mode: ProcessingMode,
    sample_rate: f32,
) -> String {
    let samples = mode.latency_samples();
    if samples == 0 {
        return format!("{desc} - 0 smp");
    }

    let sr = if sample_rate.is_finite() && sample_rate > 0.0 {
        sample_rate
    } else {
        DEFAULT_TOOLTIP_SAMPLE_RATE
    };
    let ms = (samples as f32) * 1000.0 / sr;
    let khz = sr / 1000.0;
    if (khz - khz.round()).abs() < 0.05 {
        format!(
            "{desc} - {samples} smp (~{ms:.1} ms @ {:.0} kHz)",
            khz.round()
        )
    } else {
        format!("{desc} - {samples} smp (~{ms:.1} ms @ {khz:.1} kHz)")
    }
}

/// Renders the master section controls: output gain, auto-gain, and processing mode (English default).
pub fn render_master_section(
    ui: &mut egui::Ui,
    snapshot: &ParamSnapshot,
    batch: &mut GuiCommandBatch,
) {
    render_master_section_localized(
        ui,
        snapshot,
        Translations::for_language(Language::English),
        batch,
    );
}

/// Renders the master section controls using the provided translation catalog (48 kHz reference tooltips).
pub fn render_master_section_localized(
    ui: &mut egui::Ui,
    snapshot: &ParamSnapshot,
    t: &Translations,
    batch: &mut GuiCommandBatch,
) {
    render_master_section_with_rate(ui, snapshot, DEFAULT_TOOLTIP_SAMPLE_RATE, t, batch);
}

/// Renders the master section controls using the live `sample_rate` for latency tooltips.
pub fn render_master_section_with_rate(
    ui: &mut egui::Ui,
    snapshot: &ParamSnapshot,
    sample_rate: f32,
    t: &Translations,
    batch: &mut GuiCommandBatch,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.spacing_mut().slider_width = 76.0;
        ui.label(egui::RichText::new(t.master_label).strong());

        let mut output_gain = snapshot.get(ParamId(global::OUTPUT_GAIN_DB)).unwrap_or(0.0);
        if ui
            .add(
                egui::Slider::new(&mut output_gain, -24.0..=24.0)
                    .suffix(" dB")
                    .text(t.output_slider),
            )
            .changed()
        {
            batch.push(ParamId(global::OUTPUT_GAIN_DB), output_gain);
        }

        render_auto_gain_controls(ui, snapshot, t, batch);

        ui.separator();
        render_processing_mode_selector(ui, snapshot, sample_rate, t, batch);
    });
}

fn render_auto_gain_controls(
    ui: &mut egui::Ui,
    snapshot: &ParamSnapshot,
    t: &Translations,
    batch: &mut GuiCommandBatch,
) {
    let mut auto_gain = snapshot.get(ParamId(global::AUTO_GAIN)).unwrap_or(0.0) >= 0.5;
    let auto_gain_label = if auto_gain {
        let comp = snapshot
            .get(ParamId(global::AUTO_GAIN_COMPENSATION_DB))
            .unwrap_or(0.0);
        format!("{} ({comp:+.1} dB)", t.auto_gain)
    } else {
        t.auto_gain.to_string()
    };

    if ui
        .checkbox(&mut auto_gain, auto_gain_label)
        .on_hover_text(t.tooltip_auto_gain)
        .changed()
    {
        batch.push(
            ParamId(global::AUTO_GAIN),
            if auto_gain { 1.0 } else { 0.0 },
        );
    }

    if !auto_gain {
        return;
    }

    let is_safe = snapshot.get(ParamId(global::AUTO_GAIN_MODE)).unwrap_or(0.0) >= 0.5;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if ui
            .selectable_label(!is_safe, t.auto_gain_mode_perceptual)
            .on_hover_text(t.tooltip_auto_gain_perceptual)
            .clicked()
            && is_safe
        {
            batch.push(ParamId(global::AUTO_GAIN_MODE), 0.0);
        }
        if ui
            .selectable_label(is_safe, t.auto_gain_mode_safe)
            .on_hover_text(t.tooltip_auto_gain_safe)
            .clicked()
            && !is_safe
        {
            batch.push(ParamId(global::AUTO_GAIN_MODE), 1.0);
        }
    });
}

fn render_processing_mode_selector(
    ui: &mut egui::Ui,
    snapshot: &ParamSnapshot,
    sample_rate: f32,
    t: &Translations,
    batch: &mut GuiCommandBatch,
) {
    let active_mode = snapshot
        .get(ParamId(global::PROCESSING_MODE))
        .filter(|v| v.is_finite())
        .and_then(|v| ProcessingMode::from_raw(v.round().clamp(0.0, 2.0) as u32))
        .unwrap_or(ProcessingMode::MinimumPhase);

    let modes = [
        (
            ProcessingMode::MinimumPhase,
            t.processing_mode_zero,
            t.tooltip_processing_mode_zero,
        ),
        (
            ProcessingMode::LinearPhaseLow,
            t.processing_mode_linear_low,
            t.tooltip_processing_mode_linear_low,
        ),
        (
            ProcessingMode::LinearPhaseHigh,
            t.processing_mode_linear_high,
            t.tooltip_processing_mode_linear_high,
        ),
    ];

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (mode, label, tooltip_desc) in modes {
            let selected = active_mode == mode;
            let response = ui.selectable_label(selected, label);
            if response.clicked() && !selected {
                batch.push(ParamId(global::PROCESSING_MODE), mode.to_raw() as f32);
            }
            response.on_hover_ui(|ui| {
                ui.label(format_processing_mode_tooltip(
                    tooltip_desc,
                    mode,
                    sample_rate,
                ));
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_processing_mode_tooltip_sample_rates() {
        assert_eq!(
            format_processing_mode_tooltip("Zero", ProcessingMode::MinimumPhase, 48_000.0),
            "Zero - 0 smp"
        );
        assert_eq!(
            format_processing_mode_tooltip("Lin Low", ProcessingMode::LinearPhaseLow, 48_000.0),
            "Lin Low - 1024 smp (~21.3 ms @ 48 kHz)"
        );
        assert_eq!(
            format_processing_mode_tooltip("Lin High", ProcessingMode::LinearPhaseHigh, 48_000.0),
            "Lin High - 2048 smp (~42.7 ms @ 48 kHz)"
        );
        assert_eq!(
            format_processing_mode_tooltip("Lin Low", ProcessingMode::LinearPhaseLow, 44_100.0),
            "Lin Low - 1024 smp (~23.2 ms @ 44.1 kHz)"
        );
    }

    #[test]
    fn test_format_processing_mode_tooltip_invalid_rate_falls_back_to_48khz() {
        assert_eq!(
            format_processing_mode_tooltip("Lin Low", ProcessingMode::LinearPhaseLow, 0.0),
            "Lin Low - 1024 smp (~21.3 ms @ 48 kHz)"
        );
        assert_eq!(
            format_processing_mode_tooltip("Lin High", ProcessingMode::LinearPhaseHigh, f32::NAN),
            "Lin High - 2048 smp (~42.7 ms @ 48 kHz)"
        );
    }

    #[test]
    fn test_clicking_processing_mode_emits_param_command() {
        let ctx = egui::Context::default();
        let snapshot = ParamSnapshot::new(Vec::new());

        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut batch = GuiCommandBatch::new();
                render_master_section_with_rate(
                    ui,
                    &snapshot,
                    48_000.0,
                    Translations::for_language(Language::English),
                    &mut batch,
                );
            });
        });

        let target_pos = out
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::epaint::Shape::Text(text_shape) if text_shape.galley.text() == "Lin Low" => {
                    Some(text_shape.visual_bounding_rect().center())
                }
                _ => None,
            })
            .expect("Lin Low button text must be painted");

        for (pressed, expect_cmd) in [(true, false), (false, true)] {
            let input = egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(target_pos),
                    egui::Event::PointerButton {
                        pos: target_pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            };
            let mut batch = GuiCommandBatch::new();
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_master_section_with_rate(
                        ui,
                        &snapshot,
                        48_000.0,
                        Translations::for_language(Language::English),
                        &mut batch,
                    );
                });
            });
            if expect_cmd {
                assert_eq!(batch.commands().len(), 1);
                assert_eq!(
                    batch.commands()[0].param_id,
                    ParamId(global::PROCESSING_MODE)
                );
                assert_eq!(batch.commands()[0].value, 1.0);
            }
        }
    }
}
