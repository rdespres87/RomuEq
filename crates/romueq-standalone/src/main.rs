/// Standalone entry point for RomuEq GUI.
///
/// Creates the RomuEq application and runs it with eframe.
/// On macOS, intercepts system audio output via BlackHole 2ch and restores
/// hardware output upon exit.
#[cfg(target_os = "macos")]
mod macos_audio;

use romueq_gui::GuiApp;
use romueq_gui::standalone;

/// The main RomuEq application.
struct RomuEqApp;

impl GuiApp for RomuEqApp {
    fn new() -> Self {
        Self
    }

    fn window_title(&self) -> &str {
        "RomuEq — Parametric Equalizer"
    }

    fn window_size(&self) -> (f32, f32) {
        (1080.0, 780.0)
    }

    fn update(&mut self, client: &mut dyn romueq_gui::GuiClient) {
        let _presets = client.list_presets();
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    let mut _audio_guard = {
        // If BlackHole 2ch is not installed, prompt native admin installer
        if !macos_audio::is_blackhole_available() {
            println!("[RomuEq] BlackHole 2ch non détecté. Lancement de l'installateur natif...");
            if let Err(e) = macos_audio::install_blackhole() {
                eprintln!("[RomuEq] {e}");
            }
        }
        macos_audio::SystemAudioRoutingGuard::activate()
    };

    let app = RomuEqApp::new();
    let result = standalone::run(app);

    #[cfg(target_os = "macos")]
    {
        // Restore system audio immediately when GUI closes
        _audio_guard.restore();
    }

    if let Err(e) = result {
        eprintln!("Failed to run RomuEq: {e}");
        std::process::exit(1);
    }
}
