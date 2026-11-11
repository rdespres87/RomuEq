use romueq_gui::preset_manager::PresetManager;
use romueq_gui::workspace_ui::{HostEnvironment, WorkspaceUiState, render_workspace};
use romueq_gui::{AnalysisSnapshot, ParamSnapshot, default_8_bands, populate_factory_presets};

#[test]
fn test_workspace_ui_renders_headless() {
    let ctx = egui::Context::default();
    let mut state = WorkspaceUiState::default();
    let bands = default_8_bands();
    let sample_rate = 48000.0;
    let snapshot = ParamSnapshot::new(Vec::new());
    let analysis = AnalysisSnapshot::default();
    let mut preset_manager = PresetManager::new();
    populate_factory_presets(&mut preset_manager);

    let env = HostEnvironment::Plugin {
        host_description: "Ableton Live • VST3".to_string(),
        sample_rate: 48000.0,
        peak_levels_db: (-12.0, -12.0),
    };

    let _output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (batch, actions) = render_workspace(
                ui,
                &mut state,
                &env,
                &bands,
                sample_rate,
                &snapshot,
                &analysis,
                &preset_manager,
                "Flat",
                Some("Test Status"),
            );
            assert!(batch.is_empty());
            assert!(actions.is_empty());
        });
    });
}
