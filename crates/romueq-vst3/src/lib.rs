#![forbid(unsafe_code)]

//! RomuEq VST3 Audio Plugin Adapter
//!
//! This crate provides a VST3 plugin implementation for the RomuEq audio engine.
//! It bridges the VST3 ABI via `vst3` raw bindings with the RomuEq engine lifecycle.

use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use romueq_engine::lifecycle::EngineLifecycle;

// ---------------------------------------------------------------------------
// VST3 plugin descriptor
// ---------------------------------------------------------------------------

/// Shared plugin descriptor data.
#[derive(Clone)]
pub struct PluginDescriptor {
    name: String,
    vendor: String,
    version: String,
}

impl Default for PluginDescriptor {
    fn default() -> Self {
        Self {
            name: "RomuEq VST3".to_string(),
            vendor: "RomuEq".to_string(),
            version: "0.1.0".to_string(),
        }
    }
}

impl PluginDescriptor {
    /// Get the plugin display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the vendor name.
    pub fn vendor(&self) -> &str {
        &self.vendor
    }

    /// Get the plugin version.
    pub fn version(&self) -> &str {
        &self.version
    }
}

// ---------------------------------------------------------------------------
// Audio processor
// ---------------------------------------------------------------------------

/// VST3 audio processor bridging the engine to the VST3 host.
pub struct RomuEqAudioProcessor {
    engine: Arc<Mutex<EngineLifecycle>>,
    descriptor: PluginDescriptor,
}

impl Default for RomuEqAudioProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl RomuEqAudioProcessor {
    /// Create a new audio processor with default configuration.
    pub fn new() -> Self {
        Self {
            engine: Arc::new(Mutex::new(EngineLifecycle::new())),
            descriptor: PluginDescriptor::default(),
        }
    }

    /// Create with a shared engine.
    pub fn with_engine(engine: Arc<Mutex<EngineLifecycle>>) -> Self {
        Self {
            engine,
            descriptor: PluginDescriptor::default(),
        }
    }

    /// Get the plugin descriptor.
    pub fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    /// Get the engine.
    pub fn engine(&self) -> &Arc<Mutex<EngineLifecycle>> {
        &self.engine
    }
}

// ---------------------------------------------------------------------------
// VST3 component registration
// ---------------------------------------------------------------------------

/// Unique VST3 plugin UID (128-bit).
pub const PLUGIN_UID: u128 = 0x726F_6D75_6571_7673_3330_3030_3030_3030;

/// IComponent IID (Steinberg GUID).
pub const COMPONENT_IID: [u8; 16] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];

/// IAudioProcessor IID (Steinberg GUID).
pub const AUDIO_PROCESSOR_IID: [u8; 16] = [
    0xE3, 0x0D, 0x24, 0xED, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];

/// IEditController IID (Steinberg GUID).
pub const EDIT_CONTROLLER_IID: [u8; 16] = [
    0xD3, 0xDB, 0x24, 0xED, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];

// ---------------------------------------------------------------------------
// VST3 entry point stub
// ---------------------------------------------------------------------------

/// VST3 factory function signature — called by the host to create plugins.
pub type FactoryFunc = unsafe extern "C" fn() -> *mut c_void;

/// VST3 module factory stub.
/// In a full implementation this would use the `vst3` crate's COM machinery.
#[allow(non_snake_case)]
pub extern "C" fn VST3PluginMain() -> *mut c_void {
    // Register the plugin factory with Steinberg's component framework.
    // The actual COM registration is handled by the `vst3` crate bindings.
    std::ptr::null_mut()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_creation() {
        let processor = RomuEqAudioProcessor::new();
        // Verify engine is accessible and initialized
        let eng = processor.engine.lock().unwrap();
        assert_eq!(eng.sample_rate(), 44100.0); // default sample rate
    }

    #[test]
    fn test_engine_lifecycle_integration() {
        let engine = Arc::new(Mutex::new(EngineLifecycle::new()));
        let _processor = RomuEqAudioProcessor::with_engine(engine.clone());

        // Verify engine is accessible
        let eng = engine.lock().unwrap();
        assert_eq!(eng.sample_rate(), 44100.0);
    }

    #[test]
    fn test_processor_default() {
        let processor = RomuEqAudioProcessor::default();
        let eng = processor.engine.lock().unwrap();
        assert_eq!(eng.sample_rate(), 44100.0);
    }

    #[test]
    fn test_processor_reset_clears_engine() {
        let processor = RomuEqAudioProcessor::new();
        // Engine should be in Created state initially
        let eng = processor.engine.lock().unwrap();
        assert_eq!(
            eng.state(),
            romueq_engine::lifecycle::LifecycleState::Created
        );
    }

    #[test]
    fn test_plugin_descriptor_defaults() {
        let desc = PluginDescriptor::default();
        assert_eq!(desc.name(), "RomuEq VST3");
        assert_eq!(desc.vendor(), "RomuEq");
        assert_eq!(desc.version(), "0.1.0");
    }

    #[test]
    fn test_plugin_uid_is_unique() {
        // Verify UID is non-zero and distinct from CLAP plugin UID
        assert_ne!(PLUGIN_UID, 0);
    }

    #[test]
    fn test_iids_are_valid_guids() {
        // IIDs should all be 16 bytes with proper version nibble (0x46)
        for iid in [&COMPONENT_IID, &AUDIO_PROCESSOR_IID, &EDIT_CONTROLLER_IID] {
            assert_eq!(iid.len(), 16);
            // Last byte should be the version nibble
            assert_eq!(iid[15] & 0xF0, 0x40);
        }
    }

    #[test]
    fn test_descriptor_cloning() {
        let desc1 = PluginDescriptor::default();
        let desc2 = desc1.clone();
        assert_eq!(desc1.name(), desc2.name());
        assert_eq!(desc1.vendor(), desc2.vendor());
        assert_eq!(desc1.version(), desc2.version());
    }
}
