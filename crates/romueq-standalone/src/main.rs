/// Standalone entry point for RomuEq GUI.
///
/// Creates a minimal RomuEq application and runs it with eframe.
use romueq_gui::standalone;
use romueq_gui::{BandControls, GuiApp};

/// The main RomuEq application.
struct RomuEqApp;

impl GuiApp for RomuEqApp {
    fn new() -> Self {
        Self
    }

    fn window_title(&self) -> &str {
        "RomuEq"
    }

    fn window_size(&self) -> (f32, f32) {
        (1024.0, 768.0)
    }

    fn update(&mut self, client: &mut dyn romueq_gui::GuiClient) {
        let ctx = client.egui_context().clone();
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.heading("RomuEq — Multi-Band EQ");

            // Preset management section
            ui.separator();
            ui.heading("Presets");

            let presets = client.list_presets();
            if presets.is_empty() {
                ui.label("No presets loaded.");
            } else {
                ui.horizontal(|ui| {
                    for preset_name in &presets {
                        if ui.button(preset_name).clicked() {
                            let _ = client.apply_preset(preset_name);
                        }
                    }
                });
            }

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    let name = client.next_auto_name();
                    let _ = client.create_preset(&name);
                }
                if ui.button("Save...").clicked() {
                    // For now, save to a default path.
                    let presets = client.list_presets();
                    if let Some(first) = presets.first() {
                        let _ = client.save_preset_to_file(first, "/tmp/romueq_preset.bin");
                    }
                }
            });

            // Band controls
            ui.separator();
            ui.heading("Band Controls");

            let controls = BandControls::new();
            for band in 0..romueq_common::MAX_BANDS {
                ui.group(|ui| {
                    let params = controls.params(band);
                    ui.label(params.label());
                    ui.horizontal(|ui| {
                        ui.label(format!("Freq: {:?}", params.frequency_hz()));
                        ui.label(format!("Gain: {:?}", params.gain_db()));
                    });
                });
            }
        });
    }
}

fn main() {
    let app = RomuEqApp::new();
    if let Err(e) = standalone::run(app) {
        eprintln!("Failed to run RomuEq: {e}");
        std::process::exit(1);
    }
}
