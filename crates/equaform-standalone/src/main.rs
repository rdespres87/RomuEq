/// Standalone entry point for Equaform-8 GUI.
///
/// Creates the Equaform application and runs it with eframe.
/// On macOS, intercepts system audio output via the Equaform virtual driver and restores
/// the selected hardware output upon exit.
#[cfg(target_os = "macos")]
mod macos_audio;

use equaform_gui::GuiApp;
use equaform_gui::standalone;
#[cfg(target_os = "macos")]
use std::sync::{Arc, Mutex};

/// The main Equaform application.
struct EquaformApp {
    #[cfg(target_os = "macos")]
    audio_guard: Option<Arc<Mutex<macos_audio::SystemAudioRoutingGuard>>>,
}

impl GuiApp for EquaformApp {
    fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            audio_guard: None,
        }
    }

    fn window_title(&self) -> &str {
        "Equaform-8 — Parametric Equalizer"
    }

    fn window_size(&self) -> (f32, f32) {
        (1180.0, 485.0)
    }

    fn update(&mut self, client: &mut dyn equaform_gui::GuiClient) {
        let _presets = client.list_presets();
    }

    fn on_audio_started(&mut self) {
        #[cfg(target_os = "macos")]
        if let Some(ref guard) = self.audio_guard {
            if let Ok(mut guard) = guard.lock() {
                guard.activate_routing();
            }
        }
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    let audio_guard = {
        if !macos_audio::is_driver_installed_on_disk() {
            println!(
                "[Equaform-8] Virtual audio driver 'Equaform-8' not found on system. Launching setup..."
            );
            if let Err(e) = macos_audio::install_equaform_driver() {
                eprintln!("[Equaform-8] Installation of EquaformDriver skipped or failed: {e}");
            }
        }
        Arc::new(Mutex::new(macos_audio::SystemAudioRoutingGuard::prepare()))
    };

    let mut app = EquaformApp::new();
    #[cfg(target_os = "macos")]
    {
        app.audio_guard = Some(Arc::clone(&audio_guard));
    }
    let result = standalone::run(app);

    #[cfg(target_os = "macos")]
    {
        // Restore system audio immediately when GUI closes
        if let Ok(mut guard) = audio_guard.lock() {
            guard.restore();
        }
    }

    if let Err(e) = result {
        eprintln!("Failed to run Equaform-8: {e}");
        std::process::exit(1);
    }
}
