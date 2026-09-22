//! RomuEq CLAP Audio Plugin
//!
//! This crate provides a CLAP plugin implementation for the RomuEq audio engine.
//! It bridges the CLAP ABI via `clack-plugin` v0.1.x with the RomuEq engine lifecycle.

use std::ffi::CStr;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use clack_common::extensions::{
    Extension, ExtensionImplementation, PluginExtensionSide, RawExtension,
    RawExtensionImplementation,
};
use clack_common::stream::{InputStream, OutputStream};
use clack_plugin::extensions::wrapper::PluginWrapper;
use clack_plugin::prelude::*;
use clap_sys::ext::state::{clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::plugin::clap_plugin;
use clap_sys::stream::{clap_istream, clap_ostream};
use romueq_engine::lifecycle::EngineLifecycle;

// ---------------------------------------------------------------------------
// Plugin metadata / descriptor
// ---------------------------------------------------------------------------

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

/// Shared (thread-safe) data — holds the engine Arc for sharing between main thread and audio processor.
#[derive(Clone)]
pub struct RomuEqShared {
    engine: Arc<Mutex<EngineLifecycle>>,
}

impl Default for RomuEqShared {
    fn default() -> Self {
        Self {
            engine: Arc::new(Mutex::new(EngineLifecycle::new())),
        }
    }
}

impl<'a> PluginShared<'a> for RomuEqShared {}

/// Main-thread data — owns a clone of the shared engine reference.
pub struct RomuEqMainThread {
    engine: Arc<Mutex<EngineLifecycle>>,
}

impl Default for RomuEqMainThread {
    fn default() -> Self {
        Self {
            engine: Arc::new(Mutex::new(EngineLifecycle::new())),
        }
    }
}

impl<'a, S: PluginShared<'a>> PluginMainThread<'a, S> for RomuEqMainThread {}

// ---------------------------------------------------------------------------
// Audio processor — holds the engine and does DSP
// ---------------------------------------------------------------------------

/// The audio processor that bridges CLAP audio processing with the RomuEq engine.
pub struct RomuEqAudioProcessor {
    /// Shared engine reference.
    engine: Arc<Mutex<EngineLifecycle>>,
}

impl Default for RomuEqAudioProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl RomuEqAudioProcessor {
    /// Create a new audio processor with default configuration (for testing).
    pub fn new() -> Self {
        Self {
            engine: Arc::new(Mutex::new(EngineLifecycle::new())),
        }
    }

    /// Create a new audio processor with a shared engine (for the plugin).
    fn with_engine(engine: Arc<Mutex<EngineLifecycle>>) -> Self {
        Self { engine }
    }
}

impl<'a, S: PluginShared<'a>, M: PluginMainThread<'a, S>> PluginAudioProcessor<'a, S, M>
    for RomuEqAudioProcessor
{
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut M,
        shared: &'a S,
        audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        // Get the shared engine Arc from RomuEqShared.
        let romu_eq_shared = unsafe { &*(shared as *const S as *const RomuEqShared) };
        let engine = romu_eq_shared.engine.clone();

        let processor = Self::with_engine(engine);

        // Determine channel count from the host-provided audio configuration.
        // We use a default of 2 (stereo) until the port enumeration API is wired up.
        let channels = 2;

        // Prepare the engine with host-provided sample rate and block size.
        {
            let mut eng = processor.engine.lock().unwrap();
            eng.prepare(
                audio_config.sample_rate,
                audio_config.max_frames_count as usize,
                channels,
            );
        }

        Ok(processor)
    }

    fn process(
        &mut self,
        _process: Process,
        mut audio: Audio,
        _events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        // Collect all channel data into owned buffers so we can pass them
        // to the engine's slice-of-slices process() API.
        let mut all_input: Vec<Vec<f32>> = Vec::new();
        let mut all_output: Vec<Vec<f32>> = Vec::new();

        for mut port_pair in &mut audio {
            let Some(channel_pairs) = port_pair.channels()?.into_f32() else {
                continue;
            };

            for channel_pair in channel_pairs {
                match channel_pair {
                    ChannelPair::InPlace(buf) => {
                        all_input.push(buf.to_vec());
                        all_output.push(vec![0.0; buf.len()]);
                    }
                    ChannelPair::InputOutput(input, output) => {
                        all_input.push(input.to_vec());
                        all_output.push(output.to_vec());
                    }
                    ChannelPair::InputOnly(buf) => {
                        all_input.push(buf.to_vec());
                        all_output.push(vec![0.0; buf.len()]);
                    }
                    ChannelPair::OutputOnly(buf) => {
                        all_input.push(vec![0.0; buf.len()]);
                        all_output.push(buf.to_vec());
                    }
                }
            }
        }

        let num_channels = all_input.len();
        if num_channels == 0 {
            return Ok(ProcessStatus::Continue);
        }

        // Build slice-of-slices for the engine's process() API.
        let input_refs: Vec<&[f32]> = all_input.iter().map(|v| v.as_slice()).collect();
        let mut output_refs: Vec<&mut [f32]> =
            all_output.iter_mut().map(|v| v.as_mut_slice()).collect();

        // Lock the engine and run DSP processing.
        let mut engine = self.engine.lock().unwrap();
        let success = engine.process(&input_refs, &mut output_refs);
        if !success {
            return Ok(ProcessStatus::Continue);
        }

        // Copy processed output back to CLAP buffers.
        let mut idx = 0;
        for mut port_pair in &mut audio {
            let Some(channel_pairs) = port_pair.channels()?.into_f32() else {
                continue;
            };

            for channel_pair in channel_pairs {
                let out_slice = &all_output[idx];
                match channel_pair {
                    ChannelPair::InPlace(buf) => buf.copy_from_slice(out_slice),
                    ChannelPair::InputOutput(_, output) => output.copy_from_slice(out_slice),
                    ChannelPair::InputOnly(_) => {} // discard
                    ChannelPair::OutputOnly(buf) => buf.copy_from_slice(out_slice),
                }
                idx += 1;
            }
        }

        Ok(ProcessStatus::Continue)
    }

    fn deactivate(self, _main_thread: &mut M) {
        // Reset engine state before dropping.
        if let Ok(mut eng) = self.engine.lock() {
            eng.destroy();
        }
    }

    fn start_processing(&mut self) -> Result<(), PluginError> {
        // Engine is already prepared on activate; reset processing counters.
        if let Ok(mut eng) = self.engine.lock() {
            eng.reset();
        }
        Ok(())
    }

    fn stop_processing(&mut self) {
        // No-op: the engine keeps its state so that start_processing()
        // can reset and resume without re-preparing.
    }
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

// ---------------------------------------------------------------------------
// CLAP State extension implementation
// ---------------------------------------------------------------------------

/// Marker struct for the CLAP State extension.
#[derive(Copy, Clone)]
pub struct RomuEqState(RawExtension<PluginExtensionSide, clap_plugin_state>);

unsafe impl Extension for RomuEqState {
    const IDENTIFIERS: &[&CStr] = &[CLAP_EXT_STATE];
    type ExtensionSide = PluginExtensionSide;

    #[inline]
    unsafe fn from_raw(raw: RawExtension<Self::ExtensionSide>) -> Self {
        Self(raw.cast())
    }
}

/// Trait for saving/loading plugin state.
pub trait RomuEqStateImplementation {
    fn save(&mut self, save: &mut OutputStream) -> Result<(), PluginError>;
    fn load(&mut self, load: &mut InputStream) -> Result<(), PluginError>;
}

/// Implement the CLAP State extension for RomuEqPlugin.
unsafe impl<P: Plugin> ExtensionImplementation<P> for RomuEqState
where
    for<'a> P::MainThread<'a>: RomuEqStateImplementation,
{
    const IMPLEMENTATION: RawExtensionImplementation =
        RawExtensionImplementation::new(&clap_plugin_state {
            save: Some(save::<P>),
            load: Some(load::<P>),
        });
}

/// Save EQ parameters to a CLAP state stream.
unsafe extern "C" fn save<P: Plugin>(
    plugin: *const clap_plugin,
    ostream: *const clap_ostream,
) -> bool
where
    for<'a> P::MainThread<'a>: RomuEqStateImplementation,
{
    PluginWrapper::<P>::handle(plugin, |p| {
        let output = OutputStream::from_raw_mut(&mut *(ostream as *mut _));
        p.main_thread().as_mut().save(output)?;
        Ok(())
    })
    .is_some()
}

/// Load EQ parameters from a CLAP state stream.
unsafe extern "C" fn load<P: Plugin>(
    plugin: *const clap_plugin,
    istream: *const clap_istream,
) -> bool
where
    for<'a> P::MainThread<'a>: RomuEqStateImplementation,
{
    PluginWrapper::<P>::handle(plugin, |p| {
        let input = InputStream::from_raw_mut(&mut *(istream as *mut _));
        p.main_thread().as_mut().load(input)?;
        Ok(())
    })
    .is_some()
}

// ---------------------------------------------------------------------------
// Implement RomuEqStateImplementation for RomuEqMainThread
// ---------------------------------------------------------------------------

impl RomuEqStateImplementation for RomuEqMainThread {
    fn save(&mut self, save: &mut OutputStream) -> Result<(), PluginError> {
        let engine = self.engine.lock().unwrap();
        let data = engine.save_state();

        // Write all bytes to the stream.
        let mut written = 0;
        while written < data.len() {
            let n = save
                .write(&data[written..])
                .map_err(|_| PluginError::Message("Stream write error"))?;
            if n == 0 {
                return Err(PluginError::Message("Stream write returned 0"));
            }
            written += n;
        }
        Ok(())
    }

    fn load(&mut self, load: &mut InputStream) -> Result<(), PluginError> {
        // Read all bytes from the stream.
        let mut data = Vec::new();
        let mut buf = [0; 1024];
        loop {
            let n = load
                .read(&mut buf)
                .map_err(|_| PluginError::Message("Stream read error"))?;
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
        }

        let mut engine = self.engine.lock().unwrap();
        if !engine.load_state(&data) {
            return Err(PluginError::Message("Failed to load state"));
        }
        Ok(())
    }
}

impl DefaultPluginFactory for RomuEqPlugin {
    fn get_descriptor() -> PluginDescriptor {
        get_descriptor()
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Self::Shared<'_>, PluginError> {
        Ok(RomuEqShared::default())
    }

    fn new_main_thread<'a>(
        _host: HostMainThreadHandle<'a>,
        _shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        Ok(RomuEqMainThread {
            engine: Arc::new(Mutex::new(EngineLifecycle::new())),
        })
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
        assert_eq!(processor.engine.lock().unwrap().sample_rate(), 44100.0);
    }

    #[test]
    fn test_plugin_default_shared() {
        let shared = RomuEqShared::default();
        assert_eq!(shared, RomuEqShared);
    }

    #[test]
    fn test_plugin_default_main_thread() {
        let mt = RomuEqMainThread::default();
        // Verify engine was created
        assert_eq!(mt.engine.lock().unwrap().sample_rate(), 44100.0);
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
}
