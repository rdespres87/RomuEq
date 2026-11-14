/// Standalone entry point for RomuEq GUI.
///
/// Creates the RomuEq application and runs it with eframe.
/// On macOS, intercepts system audio output via the RomuEq virtual driver and restores
/// the selected hardware output upon exit.
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
        (1180.0, 485.0)
    }

    fn update(&mut self, client: &mut dyn romueq_gui::GuiClient) {
        let _presets = client.list_presets();
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    let mut _audio_guard = {
        if !macos_audio::is_romueq_driver_installed_on_disk() {
            println!(
                "[RomuEq] Virtual audio driver 'RomuEq' not found on system. Launching setup..."
            );
            if let Err(e) = macos_audio::install_romueq_driver() {
                eprintln!("[RomuEq] Installation of RomuEqDriver skipped or failed: {e}");
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
