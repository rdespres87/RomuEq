//! Master output gain and auto-gain control bar widget.

use crate::{GuiCommandBatch, ParamId, ParamSnapshot};

/// Renders the master section controls: bypass, output gain, and auto-gain.
///
/// These are global parameters that affect the entire EQ signal path.
pub fn render_master_section(
    ui: &mut egui::Ui,
    snapshot: &ParamSnapshot,
    batch: &mut GuiCommandBatch,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Master:").strong());

        // Output gain slider.
        let mut output_gain = snapshot.get(ParamId(2)).unwrap_or(0.0);
        if ui
            .add(
                egui::Slider::new(&mut output_gain, -24.0..=24.0)
                    .suffix(" dB")
                    .text("Output"),
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
            format!("Auto Gain ({comp:+.1} dB)")
        } else {
            "Auto Gain".to_string()
        };

        if ui
            .checkbox(&mut auto_gain, auto_gain_label)
            .on_hover_text("Automatic perceptual gain compensation")
            .changed()
        {
            batch.push(ParamId(3), if auto_gain { 1.0 } else { 0.0 });
        }
    });
}

