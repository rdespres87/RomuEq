//! Master output gain and auto-gain control bar widget.

use crate::i18n::{Language, Translations};
use crate::{GuiCommandBatch, ParamId, ParamSnapshot};

/// Renders the master section controls: output gain and auto-gain (English default).
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

/// Renders the master section controls using the provided translation catalog.
pub fn render_master_section_localized(
    ui: &mut egui::Ui,
    snapshot: &ParamSnapshot,
    t: &Translations,
    batch: &mut GuiCommandBatch,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(t.master_label).strong());

        // Output gain slider.
        let mut output_gain = snapshot.get(ParamId(2)).unwrap_or(0.0);
        if ui
            .add(
                egui::Slider::new(&mut output_gain, -24.0..=24.0)
                    .suffix(" dB")
                    .text(t.output_slider),
            )
            .changed()
        {
            batch.push(ParamId(2), output_gain);
        }

        // Auto-gain checkbox.
        let auto_gain_val = snapshot.get(ParamId(3)).unwrap_or(0.0);
        let mut auto_gain = auto_gain_val >= 0.5;

        let auto_gain_label = if auto_gain {
            let comp = snapshot
                .get(ParamId(equaform_common::global::AUTO_GAIN_COMPENSATION_DB))
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
            batch.push(ParamId(3), if auto_gain { 1.0 } else { 0.0 });
        }
    });
}
