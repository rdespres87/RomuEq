//! RomuEq CLAP Audio Plugin
//!
//! This crate provides a CLAP plugin implementation for the RomuEq audio engine.
//! It bridges the CLAP ABI via `clack-plugin` v0.1.x with the RomuEq engine lifecycle.

use clack_plugin::prelude::*;
use romueq_engine::lifecycle::EngineLifecycle;

// ---------------------------------------------------------------------------
// Plugin metadata / descriptor
// ---------------------------------------------------------------------------

/// Compute a CLAP-style 32-bit identifier from a string (DJB2 variant).
#[allow(dead_code)]
fn compute_clap_id(s: &str) -> u32 {
    let mut hash: u32 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
    }
    hash
}

/// Stable descriptor shared across all plugin instances.
fn get_descriptor() -> PluginDescriptor {
    let mut desc = PluginDescriptor::new("com.romueq.plugin", "RomuEq");
    desc = desc.with_vendor("RomuEq");
    desc = desc.with_version("0.1.0");
    desc = desc.with_description("Real-time spectral equalizer plugin");
    desc
}

// ---------------------------------------------------------------------------
// Thread-model types — unit structs implementing the clack-plugin traits
// ---------------------------------------------------------------------------

/// Shared (thread-safe) data — no per-instance state needed yet.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct RomuEqShared;

impl<'a> PluginShared<'a> for RomuEqShared {}

/// Main-thread data — no per-instance state needed yet.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct RomuEqMainThread;

impl<'a, S: PluginShared<'a>> PluginMainThread<'a, S> for RomuEqMainThread {}

// ---------------------------------------------------------------------------
// Audio processor — holds the engine and does DSP
// ---------------------------------------------------------------------------

/// The audio processor that bridges CLAP audio processing with the RomuEq engine.
pub struct RomuEqAudioProcessor {
    /// Internal engine managing equalization parameters and processing.
    engine: EngineLifecycle,
}

impl RomuEqAudioProcessor {
    /// Create a new audio processor with default configuration.
    fn new() -> Self {
        Self {
            engine: EngineLifecycle::new(),
        }
    }
}

impl<'a, S: PluginShared<'a>, M: PluginMainThread<'a, S>> PluginAudioProcessor<'a, S, M>
    for RomuEqAudioProcessor
{
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut M,
        _shared: &'a S,
        _audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        Ok(Self::new())
    }

    fn process(
        &mut self,
        _process: Process,
        _audio: Audio,
        _events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        // Process audio through the engine.
        // TODO: wire _audio buffers to engine.process() when EngineLifecycle
        //       exposes an audio-processing API.
        Ok(ProcessStatus::Sleep)
    }

    fn deactivate(mut self, _main_thread: &mut M) {
        self.engine.destroy();
    }

    fn start_processing(&mut self) -> Result<(), PluginError> {
        Ok(())
    }

    fn stop_processing(&mut self) {}
}

// ---------------------------------------------------------------------------
// RomuEqPlugin — the marker type tying together the thread-model types
// ---------------------------------------------------------------------------

/// Marker struct that defines the plugin's thread model and factory behavior.
#[derive(Default)]
pub struct RomuEqPlugin;

impl Plugin for RomuEqPlugin {
    type Shared<'a> = RomuEqShared;
    type MainThread<'a> = RomuEqMainThread;
    type AudioProcessor<'a> = RomuEqAudioProcessor;

    fn declare_extensions(
        _builder: &mut PluginExtensions<Self>,
        _shared: Option<&Self::Shared<'_>>,
    ) {
        // Register CLAP extensions here (e.g. parameter, state, audio ports).
    }
}

impl DefaultPluginFactory for RomuEqPlugin {
    fn get_descriptor() -> PluginDescriptor {
        get_descriptor()
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
        Ok(RomuEqShared)
    }

    fn new_main_thread<'a>(
        _host: HostMainThreadHandle<'a>,
        _shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        Ok(RomuEqMainThread)
    }
}

// ---------------------------------------------------------------------------
// Entry point — the CLAP ABI entry function
// ---------------------------------------------------------------------------

clack_export_entry!(SinglePluginEntry::<RomuEqPlugin>);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_creation() {
        let processor = RomuEqAudioProcessor::new();
        assert_eq!(processor.engine.sample_rate(), 44100.0);
    }

    #[test]
    fn test_plugin_default_shared() {
        let shared = RomuEqShared::default();
        assert_eq!(shared, RomuEqShared);
    }

    #[test]
    fn test_plugin_default_main_thread() {
        let mt = RomuEqMainThread::default();
        assert_eq!(mt, RomuEqMainThread);
    }

    #[test]
    fn test_plugin_descriptor() {
        let desc = get_descriptor();
        assert!(desc.id().is_some());
        assert!(desc.name().is_some());
    }

    #[test]
    fn test_plugin_marker_default() {
        let _plugin = RomuEqPlugin::default();
    }

    #[test]
    fn test_clap_id_computation() {
        let id_str = "com.romueq.plugin";
        let hash = compute_clap_id(id_str);
        assert_ne!(hash, 0);
        // Verify the computed hash is valid (not u32::MAX)
        assert_ne!(hash, u32::MAX);
    }

    #[test]
    fn test_descriptor_has_required_fields() {
        let desc = get_descriptor();
        assert!(desc.id().unwrap().to_bytes() == b"com.romueq.plugin");
        assert!(desc.name().unwrap().to_bytes() == b"RomuEq");
    }
}
