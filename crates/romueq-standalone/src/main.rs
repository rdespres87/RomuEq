/// Standalone entry point for RomuEq GUI.
///
/// Creates a minimal RomuEq application and runs it with eframe.
use romueq_gui::GuiApp;
use romueq_gui::standalone;

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
        // All rendering is handled by the StandaloneRunner.
        // This method only updates local state.
        let _presets = client.list_presets();
    }
}

fn main() {
    let app = RomuEqApp::new();
    if let Err(e) = standalone::run(app) {
        eprintln!("Failed to run RomuEq: {e}");
        std::process::exit(1);
    }
}
