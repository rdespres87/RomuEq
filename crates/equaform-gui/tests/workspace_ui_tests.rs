use equaform_gui::is_factory_preset;
use equaform_gui::preset_manager::PresetManager;
use equaform_gui::workspace_ui::{
    HostEnvironment, WorkspaceUiContext, WorkspaceUiState, render_workspace,
};
use equaform_gui::{
    AnalysisSnapshot, ParamSnapshot, default_8_bands, populate_factory_presets,
    render_vertical_stereo_vu_meter,
};

#[test]
fn test_inspect_layout_geometry() {
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

    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1180.0, 485.0),
        )),
        ..Default::default()
    };

    let _output = ctx.run(raw_input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let initial_min_rect = ui.min_rect();
            let ws_ctx = WorkspaceUiContext {
                env: &env,
                band_params: &bands,
                sample_rate,
                snapshot: &snapshot,
                analysis: &analysis,
                preset_manager: &preset_manager,
                current_preset: "Flat",
                status_message: Some("Test Status"),
            };
            let (_batch, _actions) = render_workspace(ui, &mut state, &ws_ctx);

            let rendered_rect = ui.min_rect();
            // Window width is 1180.0, height is 485.0. Default central panel padding is 8.0px.
            // Verify left, right, top, and bottom margins are strictly symmetric (8.0px each).
            assert_eq!(
                rendered_rect.min.x, 8.0,
                "Left margin must be exactly 8.0px"
            );
            assert_eq!(rendered_rect.min.y, 8.0, "Top margin must be exactly 8.0px");
            assert_eq!(
                rendered_rect.max.x, 1172.0,
                "Right margin must be exactly 8.0px (1180.0 - 1172.0)"
            );
            assert!(
                rendered_rect.max.y <= 477.0,
                "Bottom margin must be at least 8.0px (rendered max.y was {})",
                rendered_rect.max.y
            );
            assert_eq!(
                initial_min_rect, rendered_rect,
                "Layout must not overflow the central panel rect"
            );
        });
    });
}

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
            let ws_ctx = WorkspaceUiContext {
                env: &env,
                band_params: &bands,
                sample_rate,
                snapshot: &snapshot,
                analysis: &analysis,
                preset_manager: &preset_manager,
                current_preset: "Flat",
                status_message: Some("Test Status"),
            };
            let (batch, actions) = render_workspace(ui, &mut state, &ws_ctx);
            assert!(batch.is_empty());
            assert!(actions.is_empty());
        });
    });
}

#[test]
fn test_factory_preset_protection() {
    assert!(is_factory_preset("Flat"));
    assert!(is_factory_preset("Bass Boost (+6 dB)"));
    assert!(is_factory_preset("Electronic / EDM"));
    assert!(is_factory_preset("Rock / Metal"));
    assert!(is_factory_preset("Night Listening"));
    assert!(is_factory_preset("Électro / EDM")); // Legacy alias

    // User-created presets must not be classified as factory presets
    assert!(!is_factory_preset("My Custom Master EQ"));
    assert!(!is_factory_preset("Vocal Chain 2"));
    assert!(!is_factory_preset(""));
}

#[test]
fn test_vertical_vu_meter_renders_headless() {
    let ctx = egui::Context::default();
    let _output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            render_vertical_stereo_vu_meter(ui, -18.5, -6.2, 180.0);
            render_vertical_stereo_vu_meter(ui, 0.5, -95.0, 180.0); // clipping and silence
        });
    });
}

#[test]
fn test_standalone_workspace_rendering() {
    let ctx = egui::Context::default();
    let mut state = WorkspaceUiState::default();
    let bands = default_8_bands();
    let sample_rate = 48000.0;
    let snapshot = ParamSnapshot::new(Vec::new());
    let analysis = AnalysisSnapshot::default();
    let mut preset_manager = PresetManager::new();
    populate_factory_presets(&mut preset_manager);

    let env = HostEnvironment::Standalone {
        audio_active: true,
        current_output: "External Headphones".to_string(),
        available_outputs: vec![
            "Internal Speakers".to_string(),
            "External Headphones".to_string(),
        ],
        peak_levels_db: (-24.0, -18.0),
    };

    let _output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let ws_ctx = WorkspaceUiContext {
                env: &env,
                band_params: &bands,
                sample_rate,
                snapshot: &snapshot,
                analysis: &analysis,
                preset_manager: &preset_manager,
                current_preset: "Modern Pop",
                status_message: None,
            };
            let (batch, actions) = render_workspace(ui, &mut state, &ws_ctx);
            assert!(batch.is_empty());
            assert!(actions.is_empty());
        });
    });
}

#[test]
fn test_master_bar_fits_within_1180x560_across_all_languages_with_auto_gain_on() {
    use equaform_common::global;
    use equaform_gui::i18n::Language;
    use equaform_gui::{ParamId, ParamValue};

    let bands = default_8_bands();
    let sample_rate = 48_000.0;
    let snapshot = ParamSnapshot::new(vec![
        ParamValue {
            param_id: ParamId(global::AUTO_GAIN),
            value: 1.0,
        },
        ParamValue {
            param_id: ParamId(global::AUTO_GAIN_COMPENSATION_DB),
            value: -2.4,
        },
        ParamValue {
            param_id: ParamId(global::AUTO_GAIN_MODE),
            value: 1.0,
        },
        ParamValue {
            param_id: ParamId(global::PROCESSING_MODE),
            value: 2.0,
        },
    ]);
    let analysis = AnalysisSnapshot::default();
    let mut preset_manager = PresetManager::new();
    populate_factory_presets(&mut preset_manager);

    let env = HostEnvironment::Plugin {
        host_description: "Ableton Live • VST3".to_string(),
        sample_rate,
        peak_levels_db: (-12.0, -12.0),
    };

    for lang in Language::ALL {
        let ctx = egui::Context::default();
        let mut state = WorkspaceUiState {
            language: lang,
            ..Default::default()
        };
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1180.0, 560.0),
            )),
            ..Default::default()
        };

        let _output = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let initial_min_rect = ui.min_rect();
                let ws_ctx = WorkspaceUiContext {
                    env: &env,
                    band_params: &bands,
                    sample_rate,
                    snapshot: &snapshot,
                    analysis: &analysis,
                    preset_manager: &preset_manager,
                    current_preset: "HS7 Monitor Correction",
                    status_message: Some("Test Status"),
                };
                let (_batch, _actions) = render_workspace(ui, &mut state, &ws_ctx);

                let rendered_rect = ui.min_rect();
                assert_eq!(
                    rendered_rect.max.x, 1172.0,
                    "Horizontal overflow in {lang:?} (max.x = {})",
                    rendered_rect.max.x
                );
                assert!(
                    rendered_rect.max.y <= 552.0,
                    "Vertical overflow in {lang:?} (max.y = {})",
                    rendered_rect.max.y
                );
                assert_eq!(
                    initial_min_rect, rendered_rect,
                    "Layout must not overflow 1180x560 rect in {lang:?}"
                );
            });
        });
    }
}
